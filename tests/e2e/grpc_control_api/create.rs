//! Tests for `Create` method of gRPC [Control API].
//!
//! The specificity of these tests is such that the `Get` method is also
//! being tested at the same time.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use medea_control_api_proto::grpc::api::WebRtcPublishEndpoint_P2P;

use crate::gen_insert_str_macro;

use super::{
    create_room_req, ControlClient, MemberBuilder, WebRtcPlayEndpointBuilder,
    WebRtcPublishEndpointBuilder,
};

#[test]
fn room() {
    gen_insert_str_macro!("create-room");

    let client = ControlClient::new();
    let sids = client.create(&create_room_req(&insert_str!("{}")));
    assert_eq!(sids.len(), 2);
    sids.get(&"publisher".to_string()).unwrap();
    let responder_sid = sids.get(&"responder".to_string()).unwrap().as_str();
    assert_eq!(
        responder_sid,
        &insert_str!("ws://127.0.0.1:8080/{}/responder/test")
    );

    let mut get_resp = client.get(&insert_str!("local://{}"));
    let room = get_resp.take_room();

    let responder = room
        .get_pipeline()
        .get(&insert_str!("local://{}/responder"))
        .unwrap()
        .get_member();
    assert_eq!(responder.get_credentials(), "test");
    let responder_pipeline = responder.get_pipeline();
    assert_eq!(responder_pipeline.len(), 1);
    let responder_play = responder_pipeline
        .get(&insert_str!("local://{}/responder/play"))
        .unwrap()
        .get_webrtc_play();
    assert_eq!(
        responder_play.get_src(),
        insert_str!("local://{}/publisher/publish")
    );

    let publisher = room
        .get_pipeline()
        .get(&insert_str!("local://{}/publisher"))
        .unwrap()
        .get_member();
    assert_ne!(publisher.get_credentials(), "test");
    assert_ne!(publisher.get_credentials(), "");
    let publisher_pipeline = responder.get_pipeline();
    assert_eq!(publisher_pipeline.len(), 1);
}

#[test]
fn member() {
    gen_insert_str_macro!("create-member");

    let client = ControlClient::new();
    client.create(&create_room_req(&insert_str!("{}")));

    let add_member = MemberBuilder::default()
        .id("member")
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
        .build_request(insert_str!("local://{}/test-member"));

    let sids = client.create(&add_member);
    let e2e_test_member_sid =
        sids.get(&"test-member".to_string()).unwrap().as_str();
    assert_eq!(
        e2e_test_member_sid,
        insert_str!("ws://127.0.0.1:8080/{}/test-member/qwerty")
    );

    let member = client
        .get(&insert_str!("local://{}/test-member"))
        .take_member();
    assert_eq!(member.get_pipeline().len(), 1);
    assert_eq!(member.get_credentials(), "qwerty");
}

#[test]
fn asd() {
    WebRtcPublishEndpointBuilder::default()
        .id("publish")
        .p2p_mode(WebRtcPublishEndpoint_P2P::NEVER)
        .build()
        .unwrap()
        .build_request("local://{}/responder/publish");
}

#[test]
fn endpoint() {
    gen_insert_str_macro!("create-endpoint");

    let client = ControlClient::new();
    client.create(&create_room_req(&insert_str!("{}")));

    let create_req = WebRtcPublishEndpointBuilder::default()
        .id("publish")
        .p2p_mode(WebRtcPublishEndpoint_P2P::NEVER)
        .build()
        .unwrap()
        .build_request(insert_str!("local://{}/responder/publish"));

    let sids = client.create(&create_req);
    assert_eq!(sids.len(), 0);

    let endpoint = client
        .get(&insert_str!("local://{}/responder/publish"))
        .take_webrtc_pub();
    assert_eq!(endpoint.get_p2p(), WebRtcPublishEndpoint_P2P::NEVER);
}
