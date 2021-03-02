use std::{cell::RefCell, rc::Rc};

use actix::Context;
use function_name::named;
use futures::{channel::mpsc::unbounded, StreamExt as _};
use medea_client_api_proto::{Command, Event, IceCandidate, PeerId};
use medea_control_api_proto::grpc::api::{
    member::Credentials, web_rtc_publish_endpoint::P2p,
};

use crate::{
    grpc_control_api::{
        ControlClient, MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
        WebRtcPublishEndpointBuilder,
    },
    signalling::{SendCommand, TestMember},
    test_name,
};

/// Tests server commands validation, sending multiple invalid messages and
/// asserting that they were not relayed to other users.
#[actix_rt::test]
#[named]
async fn command_validation() {
    let control_client = Rc::new(RefCell::new(ControlClient::new().await));

    let create_room = RoomBuilder::default()
        .id(test_name!())
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .credentials(Credentials::Plain(String::from("test")))
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
                .credentials(Credentials::Plain(String::from("test")))
                .add_endpoint(
                    WebRtcPlayEndpointBuilder::default()
                        .id("play")
                        .src(format!(
                            "local://{}/publisher/publish",
                            test_name!()
                        ))
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
    let member1 = TestMember::connect(
        &format!(
            "ws://127.0.0.1:8080/ws/{}/publisher?token=test",
            test_name!()
        ),
        Some(Box::new(
            move |event: &Event,
                  _: &mut Context<TestMember>,
                  _: Vec<&Event>| {
                tx1.unbounded_send(event.clone()).unwrap();
            },
        )),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;

    let (tx2, mut rx2) = unbounded();
    TestMember::start(
        format!(
            "ws://127.0.0.1:8080/ws/{}/responder?token=test",
            test_name!()
        ),
        Some(Box::new(
            move |event: &Event,
                  _: &mut Context<TestMember>,
                  _: Vec<&Event>| {
                tx2.unbounded_send(event.clone()).unwrap();
            },
        )),
        None,
        TestMember::DEFAULT_DEADLINE,
    );

    let correct_peer_id = loop {
        if let Event::IceCandidateDiscovered { peer_id, .. } =
            rx1.next().await.unwrap()
        {
            break peer_id;
        }
    };

    while let Some(msg) = rx2.next().await {
        if let Event::IceCandidateDiscovered { .. } = msg {
            break;
        }
    }

    // Send empty candidate, that should be filtered out by server.
    member1
        .send(SendCommand(Command::SetIceCandidate {
            peer_id: correct_peer_id,
            candidate: IceCandidate {
                candidate: "".to_string(),
                sdp_m_line_index: None,
                sdp_mid: None,
            },
        }))
        .await
        .unwrap();

    // Send command with non-existant peerId, hat should be filtered out by
    // server.
    member1
        .send(SendCommand(Command::SetIceCandidate {
            peer_id: PeerId(100),
            candidate: IceCandidate {
                candidate: String::from("asdasd"),
                sdp_m_line_index: None,
                sdp_mid: None,
            },
        }))
        .await
        .unwrap();

    let correct_candidate = IceCandidate {
        candidate: String::from("this_is_valid_command"),
        sdp_m_line_index: Some(123),
        sdp_mid: None,
    };
    // Send good command, that should be relayed to second member.
    member1
        .send(SendCommand(Command::SetIceCandidate {
            peer_id: correct_peer_id,
            candidate: correct_candidate.clone(),
        }))
        .await
        .unwrap();

    // Make sure that second member only received last (valid) command.
    while let Some(msg) = rx2.next().await {
        match msg {
            Event::IceCandidateDiscovered { candidate, .. } => {
                assert_eq!(candidate, correct_candidate);
                break;
            }
            Event::LocalDescriptionApplied { .. }
            | Event::PeerCreated { .. } => (),
            _ => unreachable!("{:?}", msg),
        }
    }
}
