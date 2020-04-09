use std::{collections::HashSet, time::Duration};

use actix::{Addr, Context};
use awc::ws::CloseCode;
use futures::channel::oneshot;
use medea_client_api_proto::{
    stats::{RtcStat, RtcStatsType},
    Command, Event as RpcEvent, PeerId, PeerMetrics,
};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    callbacks::{Callbacks, GetCallbacks, GrpcCallbackServer},
    grpc_control_api::{
        ControlClient, MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
        WebRtcPublishEndpointBuilder,
    },
    signalling::{CloseSocket, SendCommand, TestMember},
};

async fn test(name: &'static str, port: u16) -> InterconnectedMembers {
    let callback_server = super::run(port);
    let mut control_client = ControlClient::new().await;

    let room = RoomBuilder::default()
        .id(name)
        .add_member(
            MemberBuilder::default()
                .id(String::from("member-1"))
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .on_start(&format!("grpc://127.0.0.1:{}", port))
                        .on_stop(&format!("grpc://127.0.0.1:{}", port))
                        .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                        .build()
                        .unwrap(),
                )
                .add_endpoint(
                    WebRtcPlayEndpointBuilder::default()
                        .id("play-member-2")
                        .src(&format!("local://{}/member-2/publish", name))
                        .on_start(&format!("grpc://127.0.0.1:{}", port))
                        .on_stop(&format!("grpc://127.0.0.1:{}", port))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .add_member(
            MemberBuilder::default()
                .id(String::from("member-2"))
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .on_start(&format!("grpc://127.0.0.1:{}", port))
                        .on_stop(&format!("grpc://127.0.0.1:{}", port))
                        .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                        .build()
                        .unwrap(),
                )
                .add_endpoint(
                    WebRtcPlayEndpointBuilder::default()
                        .id("play-member-1")
                        .src(&format!("local://{}/member-1/publish", name))
                        .on_start(&format!("grpc://127.0.0.1:{}", port))
                        .on_stop(&format!("grpc://127.0.0.1:{}", port))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();

    let create_response = control_client
        .create(room.build_request(String::new()))
        .await;

    let deadline = Some(Duration::from_secs(20));

    let (member_1_peer_id_tx, member_1_peer_id_rx) = oneshot::channel();
    let mut member_1_peer_id_tx = Some(member_1_peer_id_tx);
    let member_1_on_event =
        move |event: &RpcEvent,
              _: &mut Context<TestMember>,
              _: Vec<&RpcEvent>| {
            match event {
                RpcEvent::PeerCreated { peer_id, .. } => {
                    if let Some(tx) = member_1_peer_id_tx.take() {
                        tx.send(*peer_id).unwrap();
                    }
                }
                _ => (),
            }
        };
    let member_1_client = TestMember::connect(
        create_response.get("member-1").unwrap(),
        Box::new(member_1_on_event),
        deadline.clone(),
    )
    .await;

    let (member_2_peer_id_tx, member_2_peer_id_rx) = oneshot::channel();
    let mut member_2_peer_id_tx = Some(member_2_peer_id_tx);
    let member_2_on_event =
        move |event: &RpcEvent,
              _: &mut Context<TestMember>,
              _: Vec<&RpcEvent>| {
            match event {
                RpcEvent::PeerCreated { peer_id, .. } => {
                    if let Some(tx) = member_2_peer_id_tx.take() {
                        tx.send(*peer_id).unwrap();
                    }
                }
                _ => (),
            }
        };
    let member_2_client = TestMember::connect(
        create_response.get("member-2").unwrap(),
        Box::new(member_2_on_event),
        deadline,
    )
    .await;

    let member_1_peer_id = member_1_peer_id_rx.await.unwrap();
    let member_2_peer_id = member_2_peer_id_rx.await.unwrap();

    InterconnectedMembers {
        member_1_client,
        member_2_client,
        member_1_peer_id,
        member_2_peer_id,
        callback_server,
    }
}

struct InterconnectedMembers {
    member_1_client: Addr<TestMember>,
    member_2_client: Addr<TestMember>,
    callback_server: Addr<GrpcCallbackServer>,
    member_1_peer_id: PeerId,
    member_2_peer_id: PeerId,
}

fn set_received(stat: &mut RtcStat, received: u64) {
    if let RtcStatsType::InboundRtp(inbound) = &mut stat.stats {
        inbound.packets_received = received;
        inbound.bytes_received = received;
    }
}

fn set_sent(stat: &mut RtcStat, sent: u64) {
    if let RtcStatsType::OutboundRtp(outbound) = &mut stat.stats {
        outbound.packets_sent = sent;
        outbound.bytes_sent = sent;
    }
}

impl InterconnectedMembers {
    const IN_AUDIO_RTC_STAT: &'static str = r#"
        {
            "id": "aa",
            "timestamp": 99999999999999.0,
            "type": "inbound-rtp",
            "mediaType": "audio",
            "packetsReceived": 100,
            "bytesReceived": 100
        }"#;
    const IN_VIDEO_RTC_STAT: &'static str = r#"
        {
            "id": "bb",
            "timestamp": 99999999999999.0,
            "type": "inbound-rtp",
            "mediaType": "video",
            "packetsReceived": 100,
            "bytesReceived": 100
        }"#;
    const OUT_AUDIO_RTC_STAT: &'static str = r#"
        {
            "id": "cc",
            "timestamp": 99999999999999.0,
            "type": "outbound-rtp",
            "mediaType": "audio",
            "packetsSent": 100,
            "bytesSent": 100
        }"#;
    const OUT_VIDEO_RTC_STAT: &'static str = r#"
        {
            "id": "dd",
            "timestamp": 99999999999999.0,
            "type": "outbound-rtp",
            "mediaType": "video",
            "packetsSent": 100,
            "bytesSent": 100
        }"#;

    fn trigger_on_start(&self, sent: u64, received: u64) {
        let mut in_audio_rtc_stat: RtcStat =
            serde_json::from_str(Self::IN_AUDIO_RTC_STAT).unwrap();
        set_received(&mut in_audio_rtc_stat, received);
        let mut in_video_rtc_stat: RtcStat =
            serde_json::from_str(Self::IN_VIDEO_RTC_STAT).unwrap();
        set_received(&mut in_video_rtc_stat, received);
        let mut out_audio_rtc_stat: RtcStat =
            serde_json::from_str(Self::OUT_AUDIO_RTC_STAT).unwrap();
        set_sent(&mut out_audio_rtc_stat, sent);
        let mut out_video_rtc_stat: RtcStat =
            serde_json::from_str(Self::OUT_VIDEO_RTC_STAT).unwrap();
        set_sent(&mut out_video_rtc_stat, sent);

        self.member_1_client.do_send(SendCommand(
            Command::AddPeerConnectionMetrics {
                peer_id: self.member_1_peer_id,
                metrics: PeerMetrics::RtcStats(vec![
                    in_audio_rtc_stat.clone(),
                    in_video_rtc_stat.clone(),
                    out_audio_rtc_stat.clone(),
                    out_video_rtc_stat.clone(),
                ]),
            },
        ));
        self.member_2_client.do_send(SendCommand(
            Command::AddPeerConnectionMetrics {
                peer_id: self.member_2_peer_id,
                metrics: PeerMetrics::RtcStats(vec![
                    in_audio_rtc_stat.clone(),
                    in_video_rtc_stat.clone(),
                    out_audio_rtc_stat.clone(),
                    out_video_rtc_stat.clone(),
                ]),
            },
        ));
    }
}

#[actix_rt::test]
async fn on_start_works() {
    const NAME: &str = "on_start_works";
    let interconnected_members =
        test(NAME, super::test_ports::ENDPOINT_ON_START_WORKS).await;

    interconnected_members.trigger_on_start(100, 100);

    tokio::time::delay_for(Duration::from_millis(500)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();
    let on_start_callbacks: HashSet<_> =
        callbacks.get_on_starts().map(|req| &req.fid).collect();
    assert!(on_start_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-2/play-member-1", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-1/play-member-2", NAME)));
    assert!(on_start_callbacks.contains(&format!("{}/member-2/publish", NAME)));
}

#[actix_rt::test]
async fn on_stop_works_on_leave() {
    const NAME: &str = "on_stop_works_on_leave";
    let interconnected_members =
        test(NAME, super::test_ports::ENDPOINT_ON_STOP_WORKS_ON_LEAVE).await;

    interconnected_members.trigger_on_start(100, 100);
    interconnected_members
        .member_2_client
        .do_send(CloseSocket(CloseCode::Normal));

    tokio::time::delay_for(Duration::from_millis(500)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();

    let on_start_callbacks: HashSet<_> =
        callbacks.get_on_starts().map(|req| &req.fid).collect();
    assert!(on_start_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_start_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-1/play-member-2", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-2/play-member-1", NAME)));

    let on_stop_callbacks: HashSet<_> =
        callbacks.get_on_stops().map(|req| &req.fid).collect();
    assert!(on_stop_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_stop_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-1/play-member-2", NAME))
    );
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-2/play-member-1", NAME))
    );
}

#[actix_rt::test]
async fn on_stop_by_timeout() {
    const NAME: &str = "on_stop_by_timeout";
    let interconnected_members =
        test(NAME, super::test_ports::ENDPOINT_ON_STOP_BY_TIMEOUT).await;

    interconnected_members.trigger_on_start(100, 100);

    tokio::time::delay_for(Duration::from_secs(12)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();

    let on_start_callbacks: HashSet<_> =
        callbacks.get_on_starts().map(|req| &req.fid).collect();
    assert!(on_start_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_start_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-1/play-member-2", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-2/play-member-1", NAME)));

    let on_stop_callbacks: HashSet<_> =
        callbacks.get_on_stops().map(|req| &req.fid).collect();
    assert!(on_stop_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_stop_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-1/play-member-2", NAME))
    );
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-2/play-member-1", NAME))
    );
}

#[actix_rt::test]
async fn on_stop_on_contradiction() {
    const NAME: &str = "on_stop_on_contradiction";
    let interconnected_members =
        test(NAME, super::test_ports::ENDPOINT_ON_STOP_ON_CONTRADICTION).await;

    let in_audio_rtc_stat: RtcStat =
        serde_json::from_str(InterconnectedMembers::IN_AUDIO_RTC_STAT).unwrap();
    let in_video_rtc_stat: RtcStat =
        serde_json::from_str(InterconnectedMembers::IN_VIDEO_RTC_STAT).unwrap();
    let out_audio_rtc_stat: RtcStat =
        serde_json::from_str(InterconnectedMembers::OUT_AUDIO_RTC_STAT)
            .unwrap();
    let out_video_rtc_stat: RtcStat =
        serde_json::from_str(InterconnectedMembers::OUT_VIDEO_RTC_STAT)
            .unwrap();
    interconnected_members.member_1_client.do_send(SendCommand(
        Command::AddPeerConnectionMetrics {
            peer_id: interconnected_members.member_1_peer_id,
            metrics: PeerMetrics::RtcStats(vec![
                in_audio_rtc_stat.clone(),
                in_video_rtc_stat.clone(),
                out_audio_rtc_stat.clone(),
                out_video_rtc_stat.clone(),
            ]),
        },
    ));

    tokio::time::delay_for(Duration::from_secs(6)).await;
    interconnected_members.member_1_client.do_send(SendCommand(
        Command::AddPeerConnectionMetrics {
            peer_id: interconnected_members.member_1_peer_id,
            metrics: PeerMetrics::RtcStats(vec![
                in_audio_rtc_stat.clone(),
                in_video_rtc_stat.clone(),
                out_audio_rtc_stat.clone(),
                out_video_rtc_stat.clone(),
            ]),
        },
    ));

    tokio::time::delay_for(Duration::from_secs(10)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();

    let on_start_callbacks: HashSet<_> =
        callbacks.get_on_starts().map(|req| &req.fid).collect();
    assert!(on_start_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_start_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-1/play-member-2", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-2/play-member-1", NAME)));

    let on_stop_callbacks: HashSet<_> =
        callbacks.get_on_stops().map(|req| &req.fid).collect();
    assert!(on_stop_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_stop_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-1/play-member-2", NAME))
    );
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-2/play-member-1", NAME))
    );
}

#[actix_rt::test]
async fn on_stop_didnt_fires_while_all_normal() {
    const NAME: &str = "on_stop_didnt_fires_while_all_normal";
    let interconnected_members = test(
        NAME,
        super::test_ports::ENDPOINT_ON_STOP_DIDNT_FIRES_WHILE_ALL_NORMAL,
    )
    .await;
    interconnected_members.trigger_on_start(100, 100);

    tokio::time::delay_for(Duration::from_secs(6)).await;
    interconnected_members.trigger_on_start(3000, 3000);

    tokio::time::delay_for(Duration::from_secs(10)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(callbacks.get_on_stops().count(), 0);
}
