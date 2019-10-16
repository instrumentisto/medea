//! Tests for `Create` method of gRPC [Control API].
//!
//! The specificity of these tests is such that the `Get` method is also
//! being tested at the same time.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use medea_control_api_proto::grpc::api::WebRtcPublishEndpoint_P2P;

use crate::gen_insert_str_macro;

use medea::api::control::error_codes::ErrorCode;

use super::{
    create_room_req, ControlClient, MemberBuilder, RoomBuilder,
    WebRtcPlayEndpointBuilder, WebRtcPublishEndpointBuilder,
};

mod room {
    use super::*;

    #[test]
    fn room() {
        gen_insert_str_macro!("create-room");

        let client = ControlClient::new();
        let sids = client.create(&create_room_req(&insert_str!("{}")));
        assert_eq!(sids.len(), 2);
        sids.get(&"publisher".to_string()).unwrap();
        let responder_sid =
            sids.get(&"responder".to_string()).unwrap().as_str();
        assert_eq!(
            responder_sid,
            &insert_str!("ws://127.0.0.1:8080/ws/{}/responder/test")
        );

        let mut get_resp = client.get(&insert_str!("{}"));
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
            insert_str!("local://{}/publisher/publish")
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
        gen_insert_str_macro!("cant_create_rooms_with_duplicate_ids");

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(insert_str!("{}"))
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
        gen_insert_str_macro!("element_id_mismatch");

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(insert_str!("{}"))
            .build()
            .unwrap()
            .build_request(insert_str!("{}"));

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
        gen_insert_str_macro!("create-member");

        let client = ControlClient::new();
        client.create(&create_room_req(&insert_str!("{}")));

        let add_member = MemberBuilder::default()
            .id("test-member")
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

        let sids = client.create(&add_member);
        let e2e_test_member_sid =
            sids.get(&"test-member".to_string()).unwrap().as_str();
        assert_eq!(
            e2e_test_member_sid,
            insert_str!("ws://127.0.0.1:8080/ws/{}/test-member/qwerty")
        );

        let member = client.get(&insert_str!("{}/test-member")).take_member();
        assert_eq!(member.get_pipeline().len(), 1);
        assert_eq!(member.get_credentials(), "qwerty");
    }

    #[test]
    fn cant_create_member_in_non_existent_room() {
        gen_insert_str_macro!("cant_create_member_in_non_existent_room");

        let client = ControlClient::new();

        let create_member = MemberBuilder::default()
            .id("caller")
            .build()
            .unwrap()
            .build_request(insert_str!("{}"));

        if let Err(err) = client.try_create(&create_member) {
            assert_eq!(err.code, ErrorCode::RoomNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn cant_create_members_with_duplicate_ids() {
        gen_insert_str_macro!("cant_create_members_with_duplicate_ids");

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(insert_str!("{}"))
            .build()
            .unwrap()
            .build_request("");

        client.create(&create_room);

        let create_member = MemberBuilder::default()
            .id("caller")
            .build()
            .unwrap()
            .build_request(insert_str!("{}"));

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
        gen_insert_str_macro!("create-endpoint");

        let client = ControlClient::new();
        client.create(&create_room_req(&insert_str!("{}")));

        let create_req = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(WebRtcPublishEndpoint_P2P::NEVER)
            .build()
            .unwrap()
            .build_request(insert_str!("{}/responder"));

        let sids = client.create(&create_req);
        assert_eq!(sids.len(), 0);

        let endpoint = client
            .get(&insert_str!("{}/responder/publish"))
            .take_webrtc_pub();
        assert_eq!(endpoint.get_p2p(), WebRtcPublishEndpoint_P2P::NEVER);
    }

    #[test]
    fn cant_create_endpoint_in_non_existent_member() {
        gen_insert_str_macro!("cant_create_endpoint_in_non_existent_member");

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(insert_str!("{}"))
            .build()
            .unwrap()
            .build_request("");

        client.create(&create_room);

        let create_play = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
            .build()
            .unwrap()
            .build_request(insert_str!("{}/member"));

        if let Err(err) = client.try_create(&create_play) {
            assert_eq!(err.code, ErrorCode::MemberNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn cant_create_endpoint_in_non_existent_room() {
        gen_insert_str_macro!("cant_create_endpoint_in_non_existent_room");

        let client = ControlClient::new();

        let create_publish = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
            .build()
            .unwrap()
            .build_request(insert_str!("{}/member"));

        if let Err(err) = client.try_create(&create_publish) {
            assert_eq!(err.code, ErrorCode::RoomNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn cant_create_endpoints_with_duplicate_ids() {
        gen_insert_str_macro!("cant_create_endpoints_with_duplicate_ids");

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(insert_str!("{}"))
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
            .build_request(insert_str!("{}/member"));

        client.create(&create_endpoint);

        if let Err(err) = client.try_create(&create_endpoint) {
            assert_eq!(err.code, ErrorCode::EndpointAlreadyExists as u32)
        } else {
            panic!("should err")
        }
    }

    #[test]
    fn cant_create_play_endpoint_when_no_pusblish_endpoints() {
        gen_insert_str_macro!(
            "cant_create_play_endpoint_when_no_pusblish_endpoints"
        );

        let client = ControlClient::new();

        let create_room = RoomBuilder::default()
            .id(insert_str!("{}"))
            .add_member(MemberBuilder::default().id("member").build().unwrap())
            .build()
            .unwrap()
            .build_request("");

        client.create(&create_room);

        let create_endpoint = WebRtcPlayEndpointBuilder::default()
            .id("play")
            .src(insert_str!("local://{}/member/publish"))
            .build()
            .unwrap()
            .build_request(insert_str!("{}/member"));

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
