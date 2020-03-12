use std::{cell::RefCell, rc::Rc};

use actix::Context;
use futures::{channel::mpsc::*, StreamExt as _};
use medea::hashmap;
use medea_client_api_proto::{
    Command, Event, PeerConnectionState, PeerMetrics, TrackId,
};
use medea_control_api_proto::grpc::api::web_rtc_publish_endpoint::P2p;

use crate::{
    grpc_control_api::{
        ControlClient, MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
        WebRtcPublishEndpointBuilder,
    },
    signalling::{SendCommand, TestMember},
};

#[actix_rt::test]
async fn ice_restart() {
    const TEST_NAME: &str = "ice_restart";

    let control_client = Rc::new(RefCell::new(ControlClient::new().await));

    let create_room = RoomBuilder::default()
        .id(TEST_NAME)
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .credentials("test")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(P2p::Always)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .add_member(
            MemberBuilder::default()
                .id("responder")
                .credentials("test")
                .add_endpoint(
                    WebRtcPlayEndpointBuilder::default()
                        .id("play")
                        .src(format!("local://{}/publisher/publish", TEST_NAME))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request("");

    control_client.borrow_mut().create(create_room).await;

    let (tx1, mut rx1) = unbounded();
    let deadline = Some(std::time::Duration::from_secs(5));
    let member1 = TestMember::connect(
        &format!("ws://127.0.0.1:8080/ws/{}/publisher/test", TEST_NAME),
        Box::new(
            move |event: &Event,
                  _: &mut Context<TestMember>,
                  _: Vec<&Event>| {
                tx1.unbounded_send(event.clone()).unwrap();
            },
        ),
        deadline,
    )
    .await;

    let (tx2, mut rx2) = unbounded();
    let member2 = TestMember::connect(
        &format!("ws://127.0.0.1:8080/ws/{}/responder/test", TEST_NAME),
        Box::new(
            move |event: &Event,
                  _: &mut Context<TestMember>,
                  _: Vec<&Event>| {
                tx2.unbounded_send(event.clone()).unwrap();
            },
        ),
        deadline,
    )
    .await;

    let peer1_id = loop {
        if let Event::IceCandidateDiscovered { peer_id, .. } =
            rx1.next().await.unwrap()
        {
            break peer_id;
        }
    };

    let peer2_id = loop {
        if let Event::IceCandidateDiscovered { peer_id, .. } =
            rx2.next().await.unwrap()
        {
            break peer_id;
        }
    };

    // first peer connected
    member1
        .send(SendCommand(Command::AddPeerConnectionMetrics {
            peer_id: peer1_id,
            metrics: PeerMetrics::PeerConnectionStateChanged(
                PeerConnectionState::Connected,
            ),
        }))
        .await
        .unwrap();

    // second peer connected
    member2
        .send(SendCommand(Command::AddPeerConnectionMetrics {
            peer_id: peer2_id,
            metrics: PeerMetrics::PeerConnectionStateChanged(
                PeerConnectionState::Connected,
            ),
        }))
        .await
        .unwrap();

    // first peer failed
    member1
        .send(SendCommand(Command::AddPeerConnectionMetrics {
            peer_id: peer1_id,
            metrics: PeerMetrics::PeerConnectionStateChanged(
                PeerConnectionState::Failed,
            ),
        }))
        .await
        .unwrap();

    // first peer receives Event::RenegotiationStarted
    match rx1.next().await.unwrap() {
        Event::RenegotiationStarted {
            peer_id,
            ice_restart,
        } => {
            assert_eq!(peer_id, peer1_id);
            assert!(ice_restart);
        }
        _ => unreachable!(),
    }

    // second peer failed
    member2
        .send(SendCommand(Command::AddPeerConnectionMetrics {
            peer_id: peer2_id,
            metrics: PeerMetrics::PeerConnectionStateChanged(
                PeerConnectionState::Failed,
            ),
        }))
        .await
        .unwrap();

    // first peer sends renegotiation offer
    member1
        .send(SendCommand(Command::MakeSdpOffer {
            peer_id: peer1_id,
            sdp_offer: String::from("offer"),
            mids: hashmap! {
                TrackId(0) => String::from("0"),
                TrackId(1) => String::from("1"),
            },
        }))
        .await
        .unwrap();

    // second peer receives offer (and not RenegotiationStarted)
    match rx2.next().await.unwrap() {
        Event::SdpOfferMade { peer_id, sdp_offer } => {
            assert_eq!(peer_id, peer2_id);
            assert_eq!(sdp_offer, String::from("offer"));
        }
        _ => unreachable!(),
    }

    // second peer answers with SDP answer
    member2
        .send(SendCommand(Command::MakeSdpAnswer {
            peer_id: peer2_id,
            sdp_answer: String::from("answer"),
        }))
        .await
        .unwrap();

    // first peer receives answer
    match rx1.next().await.unwrap() {
        Event::SdpAnswerMade {
            peer_id,
            sdp_answer,
        } => {
            assert_eq!(peer_id, peer1_id);
            assert_eq!(sdp_answer, String::from("answer"));
        }
        _ => unreachable!(),
    }
}
