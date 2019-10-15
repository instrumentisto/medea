//! Tests for signaling which should be happen after gRPC [Control API] call.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{cell::Cell, rc::Rc, time::Duration};

use actix::{Arbiter, AsyncContext, Context, System};
use futures::future::Future as _;
use medea_client_api_proto::Event;
use medea_control_api_proto::grpc::api::WebRtcPublishEndpoint_P2P;

use crate::{
    gen_insert_str_macro, grpc_control_api::ControlClient,
    signalling::TestMember,
};

use super::{
    MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
    WebRtcPublishEndpointBuilder,
};

#[test]
fn signalling_starts_when_create_play_member_after_pub_member() {
    gen_insert_str_macro!("create-play-member-after-pub-member");
    let sys = System::new(insert_str!("{}"));

    let control_client = ControlClient::new();

    let create_room = RoomBuilder::default()
        .id(insert_str!("{}"))
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

    let peers_created = Rc::new(Cell::new(0));
    let on_event =
        move |event: &Event, ctx: &mut Context<TestMember>, _: Vec<&Event>| {
            if let Event::PeerCreated { .. } = event {
                peers_created.set(peers_created.get() + 1);
                if peers_created.get() == 2 {
                    ctx.run_later(Duration::from_secs(1), |_, _| {
                        actix::System::current().stop();
                    });
                }
            }
        };

    let deadline = Some(std::time::Duration::from_secs(5));
    Arbiter::spawn(
        TestMember::connect(
            &insert_str!("ws://127.0.0.1:8080/ws/{}/publisher/test"),
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
                        .src(insert_str!("local://{}/publisher/publish"))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap()
                .build_request(insert_str!("{}"));

            control_client.create(&create_play_member);
            TestMember::connect(
                &insert_str!("ws://127.0.0.1:8080/ws/{}/responder/qwerty"),
                Box::new(on_event),
                deadline,
            )
        })
        .map(|_| ()),
    );

    sys.run().unwrap();
}

// TODO: add signalling_starts_when_create_play_endpoint_after_pub_endpoint

#[test]
fn peers_removed_on_delete_member() {
    gen_insert_str_macro!("delete-member-check-peers-removed");
    let sys = System::new(&insert_str!("{}"));

    let control_client = ControlClient::new();

    let create_room = RoomBuilder::default()
        .id(insert_str!("{}"))
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
                        .src(insert_str!("local://{}/publisher/publish"))
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
                            .delete(&[&insert_str!("{}/responder")])
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
        &insert_str!("ws://127.0.0.1:8080/ws/{}/publisher/test"),
        Box::new(on_event.clone()),
        deadline,
    );
    TestMember::start(
        &insert_str!("ws://127.0.0.1:8080/ws/{}/responder/test"),
        Box::new(on_event),
        deadline,
    );

    sys.run().unwrap();
}
