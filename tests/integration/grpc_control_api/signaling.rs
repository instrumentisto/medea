//! Tests for signaling which should be happen after gRPC [Control API] call.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{
    cell::{Cell, RefCell},
    future::Future,
    rc::Rc,
    time::Duration,
};

use actix::{Arbiter, Context};
use function_name::named;
use futures::{channel::mpsc, StreamExt as _};
use medea::api::control::endpoints::webrtc_publish_endpoint::P2pMode;
use medea_client_api_proto::Event;
use medea_control_api_proto::grpc::api::{
    member::Credentials, web_rtc_publish_endpoint::P2p,
};
use tokio::time::timeout;

use crate::{
    grpc_control_api::ControlClient, signalling::TestMember, test_name,
};

use super::{
    MemberBuilder, RoomBuilder, WebRtcPlayEndpointBuilder,
    WebRtcPublishEndpointBuilder,
};

fn done_on_both_peers_created() -> (
    impl Fn(&Event, &mut Context<TestMember>, Vec<&Event>) + Clone,
    impl Future<Output = ()>,
) {
    let (tx, mut rx) = mpsc::channel(1);
    let peers_created = Rc::new(Cell::new(0));
    let tx = Rc::new(RefCell::new(tx));

    let fun =
        move |event: &Event, _: &mut Context<TestMember>, _: Vec<&Event>| {
            if let Event::PeerCreated { .. } = event {
                peers_created.set(peers_created.get() + 1);
                if peers_created.get() == 2 {
                    tx.borrow_mut().try_send(()).unwrap();
                }
            }
        };

    let done = async move {
        timeout(Duration::from_secs(5), rx.next())
            .await
            .unwrap()
            .unwrap();
    };

    (fun, done)
}

#[actix_rt::test]
#[named]
async fn signalling_starts_when_create_play_member_after_pub_member() {
    let mut control_client = ControlClient::new().await;

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
        .build()
        .unwrap()
        .build_request("");

    control_client.create(create_room).await;

    let (on_event, done) = done_on_both_peers_created();

    TestMember::connect(
        &format!(
            "ws://127.0.0.1:8080/ws/{}/publisher?token=test",
            test_name!()
        ),
        Some(Box::new(on_event.clone())),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;

    let create_play_member = MemberBuilder::default()
        .id("responder")
        .credentials(Credentials::Plain(String::from("qwerty")))
        .add_endpoint(
            WebRtcPlayEndpointBuilder::default()
                .id("play")
                .src(format!("local://{}/publisher/publish", test_name!()))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .build_request(test_name!());

    control_client.create(create_play_member).await;
    TestMember::connect(
        &format!(
            "ws://127.0.0.1:8080/ws/{}/responder?token=qwerty",
            test_name!()
        ),
        Some(Box::new(on_event)),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;

    done.await;
}

#[actix_rt::test]
#[named]
async fn signalling_starts_when_create_play_endpoint_after_pub_member() {
    let mut control_client = ControlClient::new().await;

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
        .build()
        .unwrap()
        .build_request("");

    control_client.create(create_room).await;

    let (on_event, done) = done_on_both_peers_created();

    TestMember::connect(
        &format!(
            "ws://127.0.0.1:8080/ws/{}/publisher?token=test",
            test_name!()
        ),
        Some(Box::new(on_event.clone())),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;

    let create_second_member = MemberBuilder::default()
        .id("responder")
        .credentials(Credentials::Plain(String::from("qwerty")))
        .build()
        .unwrap()
        .build_request(test_name!());
    control_client.create(create_second_member).await;

    let create_play = WebRtcPlayEndpointBuilder::default()
        .id("play")
        .src(format!("local://{}/publisher/publish", test_name!()))
        .build()
        .unwrap()
        .build_request(format!("{}/responder", test_name!()));

    control_client.create(create_play).await;

    TestMember::connect(
        &format!(
            "ws://127.0.0.1:8080/ws/{}/responder?token=qwerty",
            test_name!()
        ),
        Some(Box::new(on_event)),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;

    done.await;
}

#[actix_rt::test]
#[named]
async fn signalling_starts_in_loopback_scenario() {
    let mut control_client = ControlClient::new().await;

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
        .build()
        .unwrap()
        .build_request("");

    control_client.create(create_room).await;

    let (on_event, done) = done_on_both_peers_created();

    TestMember::connect(
        &format!(
            "ws://127.0.0.1:8080/ws/{}/publisher?token=test",
            test_name!()
        ),
        Some(Box::new(on_event)),
        None,
        TestMember::DEFAULT_DEADLINE,
        true,
        true,
    )
    .await;

    let create_play = WebRtcPlayEndpointBuilder::default()
        .id("play")
        .src(format!("local://{}/publisher/publish", test_name!()))
        .build()
        .unwrap()
        .build_request(format!("{}/publisher", test_name!()));

    control_client.create(create_play).await;

    done.await;
}

#[actix_rt::test]
#[named]
async fn peers_removed_on_delete_member() {
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

    let peers_created = Rc::new(Cell::new(0));
    let on_event = move |event: &Event,
                         _: &mut Context<TestMember>,
                         _: Vec<&Event>| {
        match event {
            Event::PeerCreated { .. } => {
                peers_created.set(peers_created.get() + 1);
                if peers_created.get() == 2 {
                    let client = control_client.clone();
                    Arbiter::spawn(async move {
                        client
                            .borrow_mut()
                            .delete(&[&format!("{}/responder", test_name!())])
                            .await
                            .unwrap();
                    })
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
        format!(
            "ws://127.0.0.1:8080/ws/{}/publisher?token=test",
            test_name!()
        ),
        Some(Box::new(on_event.clone())),
        None,
        deadline,
    );
    TestMember::start(
        format!(
            "ws://127.0.0.1:8080/ws/{}/responder?token=test",
            test_name!()
        ),
        Some(Box::new(on_event)),
        None,
        deadline,
    );
}
