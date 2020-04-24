//! Tests for `on_start` and `on_stop` Control API callbacks of endpoints.

use std::{collections::HashSet, time::Duration};

use actix::{Addr, Context};
use awc::ws::CloseCode;
use futures::channel::oneshot;
use medea_client_api_proto::{
    stats::{RtcStat, RtcStatsType},
    Command, Event as RpcEvent, PeerId, PeerMetrics,
};
use medea_control_api_proto::grpc::api as proto;
use tokio::time::delay_for;

use crate::{
    callbacks::{Callbacks, GetCallbacks, GrpcCallbackServer},
    grpc_control_api::{
        ControlClient, MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
        WebRtcPublishEndpointBuilder,
    },
    signalling::{CloseSocket, SendCommand, TestMember},
};

/// Creates `Room` with two interconnected `Member`s with `on_start` and
/// `on_stop` callbacks.
///
/// Returns [`InterconnectedMembers`] with which you can get callbacks of this
/// `Member`s, this `Member`s and `PeerId`s of `Peer`s created for this
/// `Member`s.
#[allow(clippy::too_many_lines)]
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
            if let RpcEvent::PeerCreated { peer_id, .. } = event {
                if let Some(tx) = member_1_peer_id_tx.take() {
                    tx.send(*peer_id).unwrap();
                }
            }
        };
    let member_1_client = TestMember::connect(
        create_response.get("member-1").unwrap(),
        Some(Box::new(member_1_on_event)),
        None,
        deadline,
    )
    .await;

    let (member_2_peer_id_tx, member_2_peer_id_rx) = oneshot::channel();
    let mut member_2_peer_id_tx = Some(member_2_peer_id_tx);
    let member_2_on_event =
        move |event: &RpcEvent,
              _: &mut Context<TestMember>,
              _: Vec<&RpcEvent>| {
            if let RpcEvent::PeerCreated { peer_id, .. } = event {
                if let Some(tx) = member_2_peer_id_tx.take() {
                    tx.send(*peer_id).unwrap();
                }
            }
        };
    let member_2_client = TestMember::connect(
        create_response.get("member-2").unwrap(),
        Some(Box::new(member_2_on_event)),
        None,
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

/// Interconnected `Member`s with `on_start` and `on_stop` callbacks.
struct InterconnectedMembers {
    /// [`TestMember`] for `member-1`. Which interconnected with `member-2`.
    member_1_client: Addr<TestMember>,

    /// [`TestMember`] for `member-2`. Which interconnected with `member-1`.
    member_2_client: Addr<TestMember>,

    /// [`GrpcCallbackServer`] which will receive all callbacks of this
    /// `Member`s.
    callback_server: Addr<GrpcCallbackServer>,

    /// `PeerId` created for `Member` with `member-1` ID.
    member_1_peer_id: PeerId,

    /// `PeerId` created for `Member` with `member-2` ID.
    member_2_peer_id: PeerId,
}

/// Sets `received_bytes` and `received_packets` fields of provided [`RtcStat`]
/// to provided `received`.
fn set_received(stat: &mut RtcStat, received: u64) {
    if let RtcStatsType::InboundRtp(inbound) = &mut stat.stats {
        inbound.packets_received = received;
        inbound.bytes_received = received;
    }
}

/// Sets `sent_bytes` and `sent_packets` fields of provided [`RtcStat`] to
/// provided `received`.
fn set_sent(stat: &mut RtcStat, sent: u64) {
    if let RtcStatsType::OutboundRtp(outbound) = &mut stat.stats {
        outbound.packets_sent = sent;
        outbound.bytes_sent = sent;
    }
}

impl InterconnectedMembers {
    /// Inbound stats for `Peer` with `audio` `mediaType`.
    const IN_AUDIO_RTC_STAT: &'static str = r#"
        {
            "id": "aa",
            "timestamp": 99999999999999.0,
            "type": "inbound-rtp",
            "mediaType": "audio",
            "packetsReceived": 100,
            "bytesReceived": 100
        }"#;
    /// Inbound stats for `Peer` with `video` `mediaType`.
    const IN_VIDEO_RTC_STAT: &'static str = r#"
        {
            "id": "bb",
            "timestamp": 99999999999999.0,
            "type": "inbound-rtp",
            "mediaType": "video",
            "packetsReceived": 100,
            "bytesReceived": 100
        }"#;
    /// Outbound stats for `Peer` with `audio` `mediaType`.
    const OUT_AUDIO_RTC_STAT: &'static str = r#"
        {
            "id": "cc",
            "timestamp": 99999999999999.0,
            "type": "outbound-rtp",
            "mediaType": "audio",
            "packetsSent": 100,
            "bytesSent": 100
        }"#;
    /// Outbound stats for `Peer` with `video` `mediaType`.
    const OUT_VIDEO_RTC_STAT: &'static str = r#"
        {
            "id": "dd",
            "timestamp": 99999999999999.0,
            "type": "outbound-rtp",
            "mediaType": "video",
            "packetsSent": 100,
            "bytesSent": 100
        }"#;

    /// Sends `outbound-rtp` and `inbound-rtp` [`RtcStats`] with `sent` and
    /// `received` packets/bytes for `Peer`s related to this
    /// [`InterconnectedMembers`].
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
                    in_audio_rtc_stat,
                    in_video_rtc_stat,
                    out_audio_rtc_stat,
                    out_video_rtc_stat,
                ]),
            },
        ));
    }
}

/// Tests that `on_start` callback fires on `outbound-rtp` and `inbound-rtp`
/// [`RtcStat`]s sending.
///
/// # Algorithm
///
/// 1. Interconnect `Member`s with `on_start` and `on_stop` callbacks.
///
/// 2. Send `outbound-rtp` and `inbound-rtp` [`RtcStat`]s from both `Member`s
///
/// 3. Check that `on_start` callbacks received for all endpoints.
#[actix_rt::test]
async fn on_start_works() {
    const NAME: &str = "on_start_works";
    let interconnected_members =
        test(NAME, super::test_ports::ENDPOINT_ON_START_WORKS).await;

    interconnected_members.trigger_on_start(100, 100);

    delay_for(Duration::from_secs(1)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();
    let on_start_callbacks: HashSet<_> =
        callbacks.filter_on_start().map(|req| &req.fid).collect();
    assert!(on_start_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-2/play-member-1", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-1/play-member-2", NAME)));
    assert!(on_start_callbacks.contains(&format!("{}/member-2/publish", NAME)));
}

/// Tests that `on_stop` callback fires when `Member` leaves media server.
///
/// # Algorithm
///
/// 1. Interconnect `Member`s with `on_start` and `on_stop` callbacks.
///
/// 2. Send `outbound-rtp` and `inbound-rtp` [`RtcStat`]s from both `Member`s
///
/// 3. Close connection of `member-2`.
///
/// 4. Check that `on_stop` callbacks received for all endpoints.
#[actix_rt::test]
async fn on_stop_works_on_leave() {
    const NAME: &str = "on_stop_works_on_leave";
    let interconnected_members =
        test(NAME, super::test_ports::ENDPOINT_ON_STOP_WORKS_ON_LEAVE).await;

    interconnected_members.trigger_on_start(100, 100);

    delay_for(Duration::from_millis(500)).await;

    interconnected_members
        .member_2_client
        .do_send(CloseSocket(CloseCode::Normal));

    delay_for(Duration::from_millis(500)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();

    let on_start_callbacks: HashSet<_> =
        callbacks.filter_on_start().map(|req| &req.fid).collect();
    assert!(on_start_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_start_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-1/play-member-2", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-2/play-member-1", NAME)));

    let on_stop_callbacks: HashSet<_> =
        callbacks.filter_on_stop().map(|req| &req.fid).collect();
    assert!(on_stop_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_stop_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-1/play-member-2", NAME))
    );
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-2/play-member-1", NAME))
    );
}

/// Tests that `on_stop` callback fires when no stats received from `Member`
/// within `10secs`.
///
/// # Algorithm
///
/// 1. Interconnect `Member`s with `on_start` and `on_stop` callbacks.
///
/// 2. Send `outbound-rtp` and `inbound-rtp` [`RtcStat`]s from both `Member`s
///
/// 3. Wait `12secs`.
///
/// 4. Check that `on_stop` callbacks received for all endpoints.
#[actix_rt::test]
async fn on_stop_by_timeout() {
    const NAME: &str = "on_stop_by_timeout";
    let interconnected_members =
        test(NAME, super::test_ports::ENDPOINT_ON_STOP_BY_TIMEOUT).await;

    interconnected_members.trigger_on_start(100, 100);

    delay_for(Duration::from_secs(7)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();

    let on_start_callbacks: HashSet<_> =
        callbacks.filter_on_start().map(|req| &req.fid).collect();
    assert!(on_start_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_start_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-1/play-member-2", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-2/play-member-1", NAME)));

    let on_stop_callbacks: HashSet<_> =
        callbacks.filter_on_stop().map(|req| &req.fid).collect();
    assert!(on_stop_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_stop_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-1/play-member-2", NAME))
    );
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-2/play-member-1", NAME))
    );
}

/// Tests that `on_stop` callbacks fires when stats of `member-1` and `member-2`
/// vary.
///
/// # Algorithm
///
/// 1. Interconnect `Member`s with `on_start` and `on_stop` callbacks.
///
/// 2. Send `outbound-rtp` and `inbound-rtp` [`RtcStat`]s only from `member-1`.
///
/// 3. Wait `6secs`.
///
/// 4. Send `outbound-rtp` and `inbound-rtp` [`RtcStat`]s only from `member-1`.
///
/// 5. Wait `10secs`.
///
/// 6. Check that `on_stop` callbacks received for all endpoints.
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

    delay_for(Duration::from_secs(3)).await;
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

    delay_for(Duration::from_secs(4)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();

    let on_start_callbacks: HashSet<_> =
        callbacks.filter_on_start().map(|req| &req.fid).collect();
    assert!(on_start_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_start_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-1/play-member-2", NAME)));
    assert!(on_start_callbacks
        .contains(&format!("{}/member-2/play-member-1", NAME)));

    let on_stop_callbacks: HashSet<_> =
        callbacks.filter_on_stop().map(|req| &req.fid).collect();
    assert!(on_stop_callbacks.contains(&format!("{}/member-1/publish", NAME)));
    assert!(on_stop_callbacks.contains(&format!("{}/member-2/publish", NAME)));
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-1/play-member-2", NAME))
    );
    assert!(
        on_stop_callbacks.contains(&format!("{}/member-2/play-member-1", NAME))
    );
}

/// Tests that `on_stop` don't fires when media traffic goes normally.
///
/// # Algorithm
///
/// 1. Interconnect `Member`s with `on_start` and `on_stop` callbacks.
///
/// 2. Send `outbound-rtp` and `inbound-rtp` [`RtcStat`]s from both `Member`s
///
/// 3. Wait `6secs`.
///
/// 4. Send `outbound-rtp` and `inbound-rtp` [`RtcStat`]s from both `Member`s
///
/// 5. Wait `10secs`.
///
/// 6. Check that no `on_stop` callbacks received.
#[actix_rt::test]
async fn on_stop_didnt_fires_while_all_normal() {
    const NAME: &str = "on_stop_didnt_fires_while_all_normal";
    let interconnected_members = test(
        NAME,
        super::test_ports::ENDPOINT_ON_STOP_DIDNT_FIRES_WHILE_ALL_NORMAL,
    )
    .await;
    interconnected_members.trigger_on_start(100, 100);

    delay_for(Duration::from_secs(2)).await;
    interconnected_members.trigger_on_start(3000, 3000);

    delay_for(Duration::from_secs(2)).await;

    let callbacks: Callbacks = interconnected_members
        .callback_server
        .send(GetCallbacks)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(callbacks.filter_on_stop().count(), 0);
}
