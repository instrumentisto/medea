//! Tests for signaling which should be happen after gRPC [Control API] call.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{cell::Cell, rc::Rc, time::Duration};

use actix::{Arbiter, AsyncContext, Context, System};
use futures::future::Future as _;
use medea_client_api_proto::Event;
use medea_control_api_proto::grpc::api::WebRtcPublishEndpoint_P2P;

use crate::{grpc_control_api::ControlClient, signalling::TestMember};

use super::{
    MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
    WebRtcPublishEndpointBuilder,
};

fn stop_on_peer_created(
) -> impl Fn(&Event, &mut Context<TestMember>, Vec<&Event>) + Clone {
    let peers_created = Rc::new(Cell::new(0));
    move |event: &Event, ctx: &mut Context<TestMember>, _: Vec<&Event>| {
        if let Event::PeerCreated { .. } = event {
            peers_created.set(peers_created.get() + 1);
            if peers_created.get() == 2 {
                ctx.run_later(Duration::from_secs(1), |_, _| {
                    actix::System::current().stop();
                });
            }
        }
    }
}

#[test]
fn signalling_starts_when_create_play_member_after_pub_member() {
    const TEST_NAME: &str = "create-play-member-after-pub-member";
    let sys = System::new(TEST_NAME);

    let control_client = ControlClient::new();

    let create_room = RoomBuilder::default()
        .id(TEST_NAME)
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .credentials("test")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request("");

    control_client.create(&create_room);

    let on_event = stop_on_peer_created();

    let deadline = Some(Duration::from_secs(5));
    Arbiter::spawn(
        TestMember::connect(
            &format!("ws://127.0.0.1:8080/ws/{}/publisher/test", TEST_NAME),
            Box::new(on_event.clone()),
            deadline,
        )
        .and_then(move |_| {
            let create_play_member = MemberBuilder::default()
                .id("responder")
                .credentials("qwerty")
                .add_endpoint(
                    WebRtcPlayEndpointBuilder::default()
                        .id("play")
                        .src(format!("local://{}/publisher/publish", TEST_NAME))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap()
                .build_request(TEST_NAME);

            control_client.create(&create_play_member);
            TestMember::connect(
                &format!(
                    "ws://127.0.0.1:8080/ws/{}/responder/qwerty",
                    TEST_NAME
                ),
                Box::new(on_event),
                deadline,
            )
        })
        .map(|_| ()),
    );

    sys.run().unwrap();
}

#[test]
fn signalling_starts_when_create_play_endpoint_after_pub_member() {
    const TEST_NAME: &str =
        "signalling_starts_when_create_play_endpoint_after_pub_member";
    let sys = System::new(TEST_NAME);

    let control_client = ControlClient::new();

    let create_room = RoomBuilder::default()
        .id(TEST_NAME)
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .credentials("test")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request("");

    control_client.create(&create_room);

    let on_event = stop_on_peer_created();

    let deadline = Some(Duration::from_secs(5));
    Arbiter::spawn(
        TestMember::connect(
            &format!("ws://127.0.0.1:8080/ws/{}/publisher/test", TEST_NAME),
            Box::new(on_event.clone()),
            deadline,
        )
        .and_then(move |_| {
            let create_second_member = MemberBuilder::default()
                .id("responder")
                .credentials("qwerty")
                .build()
                .unwrap()
                .build_request(TEST_NAME);
            control_client.create(&create_second_member);

            let create_play = WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/publisher/publish", TEST_NAME))
                .build()
                .unwrap()
                .build_request(format!("{}/responder", TEST_NAME));

            control_client.create(&create_play);

            TestMember::connect(
                &format!(
                    "ws://127.0.0.1:8080/ws/{}/responder/qwerty",
                    TEST_NAME
                ),
                Box::new(on_event),
                deadline,
            )
        })
        .map(|_| ()),
    );

    sys.run().unwrap();
}

#[test]
fn signalling_starts_in_loopback_scenario() {
    const TEST_NAME: &str = "signalling_starts_in_loopback_scenario";
    let sys = System::new(TEST_NAME);

    let control_client = ControlClient::new();

    let create_room = RoomBuilder::default()
        .id(TEST_NAME)
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .credentials("test")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request("");

    control_client.create(&create_room);

    let on_event = stop_on_peer_created();

    let deadline = Some(Duration::from_secs(5));
    Arbiter::spawn(
        TestMember::connect(
            &format!("ws://127.0.0.1:8080/ws/{}/publisher/test", TEST_NAME),
            Box::new(on_event.clone()),
            deadline,
        )
        .and_then(move |_| {
            let create_play = WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/publisher/publish", TEST_NAME))
                .build()
                .unwrap()
                .build_request(format!("{}/publisher", TEST_NAME));

            control_client.create(&create_play);
            Ok(())
        })
        .map(|_| ()),
    );

    sys.run().unwrap();
}

#[test]
fn peers_removed_on_delete_member() {
    const TEST_NAME: &str = "delete-member-check-peers-removed";
    let sys = System::new(TEST_NAME);

    let control_client = ControlClient::new();

    let create_room = RoomBuilder::default()
        .id(TEST_NAME)
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .credentials("test")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
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

    control_client.create(&create_room);

    let peers_created = Rc::new(Cell::new(0));
    let on_event =
        move |event: &Event, _: &mut Context<TestMember>, _: Vec<&Event>| {
            match event {
                Event::PeerCreated { .. } => {
                    peers_created.set(peers_created.get() + 1);
                    if peers_created.get() == 2 {
                        control_client
                            .delete(&[&format!("{}/responder", TEST_NAME)])
                            .unwrap();
                    }
                }
                Event::PeersRemoved { .. } => {
                    actix::System::current().stop();
                }
                _ => {}
            }
        };

    let deadline = Some(Duration::from_secs(5));
    TestMember::start(
        &format!("ws://127.0.0.1:8080/ws/{}/publisher/test", TEST_NAME),
        Box::new(on_event.clone()),
        deadline,
    );
    TestMember::start(
        &format!("ws://127.0.0.1:8080/ws/{}/responder/test", TEST_NAME),
        Box::new(on_event),
        deadline,
    );

    sys.run().unwrap();
}
