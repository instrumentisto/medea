use std::collections::HashMap;

use medea_control_api_proto::grpc::control_api::{
    CreateRequest, Member, Member_Element, WebRtcPlayEndpoint,
    WebRtcPublishEndpoint, WebRtcPublishEndpoint_P2P,
};

use super::{create_room_req, ControlClient};

#[test]
fn room() {
    let client = ControlClient::new();
    let sids = client.create(&create_room_req("create-room"));
    assert_eq!(sids.len(), 2);
    sids.get(&"publisher".to_string()).unwrap();
    let responder_sid = sids.get(&"responder".to_string()).unwrap().as_str();
    assert_eq!(
        responder_sid,
        "ws://0.0.0.0:8080/create-room/responder/test"
    );

    let mut get_resp = client.get("local://create-room");
    let room = get_resp.take_room();

    let responder = room
        .get_pipeline()
        .get(&"local://create-room/responder".to_string())
        .unwrap()
        .get_member();
    assert_eq!(responder.get_credentials(), "test");
    let responder_pipeline = responder.get_pipeline();
    assert_eq!(responder_pipeline.len(), 1);
    let responder_play = responder_pipeline
        .get(&"local://create-room/responder/play".to_string())
        .unwrap()
        .get_webrtc_play();
    assert_eq!(
        responder_play.get_src(),
        "local://create-room/publisher/publish"
    );

    let publisher = room
        .get_pipeline()
        .get(&"local://create-room/publisher".to_string())
        .unwrap()
        .get_member();
    assert_ne!(publisher.get_credentials(), "test");
    assert_ne!(publisher.get_credentials(), "");
    let publisher_pipeline = responder.get_pipeline();
    assert_eq!(publisher_pipeline.len(), 1);
}

#[test]
fn member() {
    let client = ControlClient::new();
    client.create(&create_room_req("create-member"));

    let create_req = {
        let mut create_member_request = CreateRequest::new();
        let mut member = Member::new();
        let mut member_pipeline = HashMap::new();

        let mut play_endpoint = WebRtcPlayEndpoint::new();
        play_endpoint
            .set_src("local://create-member/publisher/publish".to_string());
        let mut member_element = Member_Element::new();
        member_element.set_webrtc_play(play_endpoint);
        member_pipeline.insert("play".to_string(), member_element);

        member.set_credentials("qwerty".to_string());
        member.set_pipeline(member_pipeline);
        create_member_request
            .set_id("local://create-member/test-member".to_string());
        create_member_request.set_member(member);

        create_member_request
    };

    let sids = client.create(&create_req);
    let e2e_test_member_sid =
        sids.get(&"test-member".to_string()).unwrap().as_str();
    assert_eq!(
        e2e_test_member_sid,
        "ws://0.0.0.0:8080/create-member/test-member/qwerty"
    );

    let member = client
        .get("local://create-member/test-member")
        .take_member();
    assert_eq!(member.get_pipeline().len(), 1);
    assert_eq!(member.get_credentials(), "qwerty");
}

#[test]
fn endpoint() {
    let client = ControlClient::new();
    client.create(&create_room_req("create-endpoint"));

    let create_req = {
        let mut create_endpoint_request = CreateRequest::new();
        let mut endpoint = WebRtcPublishEndpoint::new();
        endpoint.set_p2p(WebRtcPublishEndpoint_P2P::NEVER);
        create_endpoint_request
            .set_id("local://create-endpoint/responder/publish".to_string());
        create_endpoint_request.set_webrtc_pub(endpoint);

        create_endpoint_request
    };
    let sids = client.create(&create_req);
    assert_eq!(sids.len(), 0);

    let endpoint = client
        .get("local://create-endpoint/responder/publish")
        .take_webrtc_pub();
    assert_eq!(endpoint.get_p2p(), WebRtcPublishEndpoint_P2P::NEVER);
}
