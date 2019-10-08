//! Tests for signaling which should be happen after gRPC [Control API] call.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{cell::Cell, collections::HashMap, rc::Rc, time::Duration};

use actix::{Arbiter, AsyncContext, Context, System};
use futures::future::Future as _;
use medea_client_api_proto::Event;
use medea_control_api_proto::grpc::api::{
    CreateRequest, Member, Member_Element, Room, Room_Element,
    WebRtcPlayEndpoint, WebRtcPublishEndpoint, WebRtcPublishEndpoint_P2P,
};

use crate::{
    format_name_macro, grpc_control_api::ControlClient, signalling::TestMember,
};

/// Creates [`CreateRequest`] for creating `Room` element with provided room ID
/// and `Member` with `WebRtcPublishEndpoint`.
///
/// # Spec of `Room` which will be created with this [`CreateRequest`]
///
/// ```yaml
/// kind: Room
/// id: {{ room_id }}
/// spec:
///   pipeline:
///     publisher:
///       kind: Member
///       credentials: test
///       spec:
///         pipeline:
///           publish:
///             kind: WebRtcPublishEndpoint
///             spec:
///               p2p: Always
/// ```
fn room_with_one_pub_member_req(room_id: &str) -> CreateRequest {
    let mut create_req = CreateRequest::new();
    let mut room = Room::new();
    let mut publisher = Member::new();
    let mut publish_endpoint = WebRtcPublishEndpoint::new();

    publish_endpoint.set_p2p(WebRtcPublishEndpoint_P2P::ALWAYS);
    let mut publish_endpoint_element = Member_Element::new();
    publish_endpoint_element.set_webrtc_pub(publish_endpoint);
    let mut publisher_pipeline = HashMap::new();
    publisher_pipeline.insert("publish".to_string(), publish_endpoint_element);
    publisher.set_pipeline(publisher_pipeline);
    publisher.set_credentials("test".to_string());

    let mut publisher_member_element = Room_Element::new();
    publisher_member_element.set_member(publisher);
    let mut room_pipeline = HashMap::new();
    room_pipeline.insert("publisher".to_string(), publisher_member_element);
    room.set_pipeline(room_pipeline);
    create_req.set_room(room.clone());
    create_req.set_id(format!("local://{}", room_id));

    create_req
}

/// Creates [`CreateRequest`] for creating `Member` element in provided Room ID.
///
/// # Spec of `Member` which will be created with this [`CreateRequest`]
///
/// ```yaml
/// kind: Member
/// id: responder
/// credentials: qwerty
/// spec:
///   pipeline:
///     play:
///       kind: WebRtcPlayEndpoint
///       spec:
///         src: "local://{{ room_id }}/publisher/publish
/// ```
fn create_play_member_req(room_id: &str) -> CreateRequest {
    let mut create_member_request = CreateRequest::new();
    let mut member = Member::new();
    let mut member_pipeline = HashMap::new();

    let mut play_endpoint = WebRtcPlayEndpoint::new();
    play_endpoint.set_src(format!("local://{}/publisher/publish", room_id));
    let mut member_element = Member_Element::new();
    member_element.set_webrtc_play(play_endpoint);
    member_pipeline.insert("play".to_string(), member_element);

    member.set_credentials("qwerty".to_string());
    member.set_pipeline(member_pipeline);
    create_member_request.set_id(format!("local://{}/responder", room_id));
    create_member_request.set_member(member);

    create_member_request
}

/// Creates [`CreateRequest`] for creating `Room` element with provided room ID.
///
/// # Spec of `Room` which will be created with this [`CreateRequest`]
///
/// ```yaml
/// kind: Room
/// id: {{ room_id }}
/// spec:
///   pipeline:
///     publisher:
///       kind: Member
///       credentials: test
///       spec:
///         pipeline:
///           publish:
///             kind: WebRtcPublishEndpoint
///             spec:
///               p2p: Always
///     responder:
///       kind: Member
///       credentials: test
///       spec:
///         pipeline:
///           play:
///             kind: WebRtcPlayEndpoint
///             spec:
///               src: "local://{{ room_id }}/publisher/publish"
/// ```
fn create_room_req(room_id: &str) -> CreateRequest {
    let mut create_req = CreateRequest::new();
    let mut room = Room::new();
    let mut publisher = Member::new();
    let mut responder = Member::new();
    let mut play_endpoint = WebRtcPlayEndpoint::new();
    let mut publish_endpoint = WebRtcPublishEndpoint::new();

    play_endpoint.set_src(format!("local://{}/publisher/publish", room_id));
    let mut play_endpoint_element = Member_Element::new();
    play_endpoint_element.set_webrtc_play(play_endpoint);
    let mut responder_pipeline = HashMap::new();
    responder_pipeline.insert("play".to_string(), play_endpoint_element);
    responder.set_pipeline(responder_pipeline);
    responder.set_credentials("test".to_string());

    publish_endpoint.set_p2p(WebRtcPublishEndpoint_P2P::ALWAYS);
    let mut publish_endpoint_element = Member_Element::new();
    publish_endpoint_element.set_webrtc_pub(publish_endpoint);
    let mut publisher_pipeline = HashMap::new();
    publisher_pipeline.insert("publish".to_string(), publish_endpoint_element);
    publisher.set_pipeline(publisher_pipeline);
    publisher.set_credentials("test".to_string());

    let mut publisher_member_element = Room_Element::new();
    publisher_member_element.set_member(publisher);
    let mut responder_member_element = Room_Element::new();
    responder_member_element.set_member(responder);
    let mut room_pipeline = HashMap::new();
    room_pipeline.insert("publisher".to_string(), publisher_member_element);
    room_pipeline.insert("responder".to_string(), responder_member_element);
    room.set_pipeline(room_pipeline);
    create_req.set_room(room.clone());
    create_req.set_id(format!("local://{}", room_id));

    create_req
}

#[test]
fn create_play_member_after_pub_member() {
    format_name_macro!("create-play-member-after-pub-member");
    let sys = System::new(format_name!("{}"));

    let control_client = ControlClient::new();
    control_client.create(&room_with_one_pub_member_req(&format_name!("{}")));

    let peers_created = Rc::new(Cell::new(0));
    let on_event =
        move |event: &Event, ctx: &mut Context<TestMember>, _: Vec<&Event>| {
            match event {
                Event::PeerCreated { .. } => {
                    peers_created.set(peers_created.get() + 1);
                    if peers_created.get() == 2 {
                        ctx.run_later(Duration::from_secs(1), |_, _| {
                            actix::System::current().stop();
                        });
                    }
                }
                _ => {}
            }
        };

    let deadline = Some(std::time::Duration::from_secs(5));
    Arbiter::spawn(
        TestMember::connect(
            &format_name!("ws://127.0.0.1:8080/ws/{}/publisher/test"),
            Box::new(on_event.clone()),
            deadline,
        )
        .and_then(move |_| {
            control_client.create(&create_play_member_req(&format_name!("{}")));
            TestMember::connect(
                &format_name!("ws://127.0.0.1:8080/ws/{}/responder/qwerty"),
                Box::new(on_event),
                deadline,
            )
        })
        .map(|_| ()),
    );

    sys.run().unwrap();
}

#[test]
fn delete_member_check_peers_removed() {
    format_name_macro!("delete-member-check-peers-removed");
    let sys = System::new(&format_name!("{}"));

    let control_client = ControlClient::new();
    control_client.create(&create_room_req(&format_name!("{}")));

    let peers_created = Rc::new(Cell::new(0));
    let on_event =
        move |event: &Event, _: &mut Context<TestMember>, _: Vec<&Event>| {
            match event {
                Event::PeerCreated { .. } => {
                    peers_created.set(peers_created.get() + 1);
                    if peers_created.get() == 2 {
                        control_client
                            .delete(&[&format_name!("local://{}/responder")]);
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
        &format_name!("ws://127.0.0.1:8080/ws/{}/publisher/test"),
        Box::new(on_event.clone()),
        deadline,
    );
    TestMember::start(
        &format_name!("ws://127.0.0.1:8080/ws/{}/responder/test"),
        Box::new(on_event),
        deadline,
    );

    sys.run().unwrap();
}
