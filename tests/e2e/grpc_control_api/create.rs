//! Tests for `Create` method of gRPC [Control API].
//!
//! The specificity of these tests is such that the `Get` method is also
//! being tested at the same time.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use medea::api::control::error_codes::ErrorCode;
use medea_control_api_proto::grpc::api::WebRtcPublishEndpoint_P2P;

use super::{
    create_room_req, ControlClient, MemberBuilder, RoomBuilder,
    WebRtcPlayEndpointBuilder, WebRtcPublishEndpointBuilder,
};

mod room {
    use super::*;

    #[test]
    fn room() {
        const TEST_NAME: &str = "create-room";

        let client = ControlClient::new();
        let sids = client.create(&create_room_req(TEST_NAME));
        assert_eq!(sids.len(), 2);
        sids.get(&"publisher".to_string()).unwrap();
        let responder_sid =
            sids.get(&"responder".to_string()).unwrap().as_str();
        assert_eq!(
            responder_sid,
            &format!("ws://127.0.0.1:8080/ws/{}/responder/test", TEST_NAME)
        );

        let mut get_resp = client.get(TEST_NAME);
        let room = get_resp.take_room();

        let responder =
            room.get_pipeline().get("responder").unwrap().get_member();
        assert_eq!(responder.get_credentials(), "test");
        let responder_pipeline = responder.get_pipeline();
        assert_eq!(responder_pipeline.len(), 1);
        let responder_play =
            responder_pipeline.get("play").unwrap().get_webrtc_play();
        assert_eq!(
            responder_play.get_src(),
            format!("local://{}/publisher/publish", TEST_NAME)
        );

        let publisher =
            room.get_pipeline().get("publisher").unwrap().get_member();
        assert_ne!(publisher.get_credentials(), "test");
        assert_ne!(publisher.get_credentials(), "");
        let publisher_pipeline = responder.get_pipeline();
        assert_eq!(publisher_pipeline.len(), 1);
    }

    #[test]
    fn cant_create_rooms_with_duplicate_ids() {
        const TEST_NAME: &str = "cant_create_rooms_with_duplicate_ids";
        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(TEST_NAME)
            .build()
            .unwrap()
            .build_request("");

        client.create(&create_room);

        if let Err(err) = client.try_create(&create_room) {
            assert_eq!(err.code, ErrorCode::RoomAlreadyExists as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn element_id_mismatch() {
        const TEST_NAME: &str = "element_id_mismatch";
        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(TEST_NAME)
            .build()
            .unwrap()
            .build_request(TEST_NAME);

        if let Err(err) = client.try_create(&create_room) {
            assert_eq!(err.code, ErrorCode::ElementIdMismatch as u32)
        } else {
            panic!("should err")
        }
    }
}

mod member {

    use super::*;

    #[test]
    fn member() {
        const TEST_NAME: &str = "create-member";

        let client = ControlClient::new();
        client.create(&create_room_req(TEST_NAME));

        let add_member = MemberBuilder::default()
            .id("test-member")
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

        let sids = client.create(&add_member);
        let e2e_test_member_sid =
            sids.get(&"test-member".to_string()).unwrap().as_str();
        assert_eq!(
            e2e_test_member_sid,
            format!("ws://127.0.0.1:8080/ws/{}/test-member/qwerty", TEST_NAME)
        );

        let member = client
            .get(&format!("{}/test-member", TEST_NAME))
            .take_member();
        assert_eq!(member.get_pipeline().len(), 1);
        assert_eq!(member.get_credentials(), "qwerty");
    }

    #[test]
    fn cant_create_member_in_non_existent_room() {
        const TEST_NAME: &str = "cant_create_member_in_non_existent_room";
        let client = ControlClient::new();

        let create_member = MemberBuilder::default()
            .id("caller")
            .build()
            .unwrap()
            .build_request(TEST_NAME);

        if let Err(err) = client.try_create(&create_member) {
            assert_eq!(err.code, ErrorCode::RoomNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn cant_create_members_with_duplicate_ids() {
        const TEST_NAME: &str = "cant_create_members_with_duplicate_ids";

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(TEST_NAME)
            .build()
            .unwrap()
            .build_request("");

        client.create(&create_room);

        let create_member = MemberBuilder::default()
            .id("caller")
            .build()
            .unwrap()
            .build_request(TEST_NAME);

        client.create(&create_member);

        if let Err(err) = client.try_create(&create_member) {
            assert_eq!(err.code, ErrorCode::MemberAlreadyExists as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn element_id_mismatch() {
        let client = ControlClient::new();

        let create_member = MemberBuilder::default()
            .id("asd")
            .build()
            .unwrap()
            .build_request("qwe/qwe");

        if let Err(err) = client.try_create(&create_member) {
            assert_eq!(err.code, ErrorCode::ElementIdMismatch as u32)
        } else {
            panic!("should err")
        }
    }
}

mod endpoint {

    use super::*;

    #[test]
    fn endpoint() {
        const TEST_NAME: &str = "create-endpoint";

        let client = ControlClient::new();
        client.create(&create_room_req(TEST_NAME));

        let create_req = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(WebRtcPublishEndpoint_P2P::NEVER)
            .build()
            .unwrap()
            .build_request(format!("{}/responder", TEST_NAME));

        let sids = client.create(&create_req);
        assert_eq!(sids.len(), 0);

        let endpoint = client
            .get(&format!("{}/responder/publish", TEST_NAME))
            .take_webrtc_pub();
        assert_eq!(endpoint.get_p2p(), WebRtcPublishEndpoint_P2P::NEVER);
    }

    #[test]
    fn cant_create_endpoint_in_non_existent_member() {
        const TEST_NAME: &str = "cant_create_endpoint_in_non_existent_member";

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(TEST_NAME)
            .build()
            .unwrap()
            .build_request("");

        client.create(&create_room);

        let create_play = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
            .build()
            .unwrap()
            .build_request(format!("{}/member", TEST_NAME));

        if let Err(err) = client.try_create(&create_play) {
            assert_eq!(err.code, ErrorCode::MemberNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn cant_create_endpoint_in_non_existent_room() {
        const TEST_NAME: &str = "cant_create_endpoint_in_non_existent_room";

        let client = ControlClient::new();

        let create_publish = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
            .build()
            .unwrap()
            .build_request(format!("{}/member", TEST_NAME));

        if let Err(err) = client.try_create(&create_publish) {
            assert_eq!(err.code, ErrorCode::RoomNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn cant_create_endpoints_with_duplicate_ids() {
        const TEST_NAME: &str = "cant_create_endpoints_with_duplicate_ids";

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(TEST_NAME)
            .add_member(MemberBuilder::default().id("member").build().unwrap())
            .build()
            .unwrap()
            .build_request("");

        client.create(&create_room);

        let create_endpoint = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
            .build()
            .unwrap()
            .build_request(format!("{}/member", TEST_NAME));

        client.create(&create_endpoint);

        if let Err(err) = client.try_create(&create_endpoint) {
            assert_eq!(err.code, ErrorCode::EndpointAlreadyExists as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn cant_create_play_endpoint_when_no_pusblish_endpoints() {
        const TEST_NAME: &str =
            "cant_create_play_endpoint_when_no_pusblish_endpoints";

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(TEST_NAME)
            .add_member(MemberBuilder::default().id("member").build().unwrap())
            .build()
            .unwrap()
            .build_request("");

        client.create(&create_room);

        let create_endpoint = WebRtcPlayEndpointBuilder::default()
            .id("play")
            .src(format!("local://{}/member/publish", TEST_NAME))
            .build()
            .unwrap()
            .build_request(format!("{}/member", TEST_NAME));

        if let Err(err) = client.try_create(&create_endpoint) {
            assert_eq!(err.code, ErrorCode::EndpointNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn element_id_mismatch() {
        let client = ControlClient::new();

        let create_endpoint = WebRtcPublishEndpointBuilder::default()
            .id("asd")
            .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
            .build()
            .unwrap()
            .build_request("qwe");

        if let Err(err) = client.try_create(&create_endpoint) {
            assert_eq!(err.code, ErrorCode::ElementIdMismatch as u32)
        } else {
            panic!("should err")
        }
    }
}
