//! Tests for `Create` method of gRPC [Control API].
//!
//! The specificity of these tests is such that the `Get` method is also
//! being tested at the same time.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use function_name::named;
use medea::api::control::error_codes::ErrorCode;
use medea_control_api_proto::grpc::api as proto;

use crate::{
    enum_eq,
    grpc_control_api::{take_member, take_room, take_webrtc_pub},
    signalling::TestMember,
    test_name,
};

use super::{
    create_room_req, ControlClient, MemberBuilder, RoomBuilder,
    WebRtcPlayEndpointBuilder, WebRtcPublishEndpointBuilder,
};

mod room {
    use super::*;

    #[actix_rt::test]
    #[named]
    async fn room() {
        let mut client = ControlClient::new().await;
        let sids = client.create(create_room_req(test_name!())).await;
        assert_eq!(sids.len(), 2);
        sids.get(&"publisher".to_string()).unwrap();
        let responder_sid =
            sids.get(&"responder".to_string()).unwrap().as_str();
        assert_eq!(
            responder_sid,
            &format!("ws://127.0.0.1:8080/ws/{}/responder/test", test_name!())
        );

        let mut room = take_room(client.get(test_name!()).await);

        let responder = room.pipeline.remove("responder").unwrap();
        let responder = match responder.el.unwrap() {
            proto::room::element::El::Member(member) => member,
            _ => panic!(),
        };
        assert_eq!(responder.credentials.as_str(), "test");
        let mut responder_pipeline = responder.pipeline;
        assert_eq!(responder_pipeline.len(), 1);
        let responder_play = responder_pipeline.remove("play").unwrap();
        let responder_play = match responder_play.el.unwrap() {
            proto::member::element::El::WebrtcPlay(play) => play,
            _ => panic!(),
        };
        assert_eq!(
            responder_play.src,
            format!("local://{}/publisher/publish", test_name!())
        );

        let publisher = room.pipeline.remove("publisher").unwrap();
        let publisher = match publisher.el.unwrap() {
            proto::room::element::El::Member(member) => member,
            _ => panic!(),
        };
        assert_ne!(publisher.credentials.as_str(), "test");
        assert_ne!(publisher.credentials.as_str(), "");
        let publisher_pipeline = publisher.pipeline;
        assert_eq!(publisher_pipeline.len(), 1);
    }

    #[actix_rt::test]
    #[named]
    async fn cant_create_rooms_with_duplicate_ids() {
        let mut client = ControlClient::new().await;

        let create_room = RoomBuilder::default()
            .id(test_name!())
            .build()
            .unwrap()
            .build_request("");

        client.create(create_room.clone()).await;

        if let Err(err) = client.try_create(create_room).await {
            assert_eq!(err.code, ErrorCode::RoomAlreadyExists as u32)
        } else {
            panic!("should err")
        }
    }

    #[actix_rt::test]
    #[named]
    async fn element_id_mismatch() {
        let mut client = ControlClient::new().await;

        let create_room = RoomBuilder::default()
            .id(test_name!())
            .build()
            .unwrap()
            .build_request(test_name!());

        if let Err(err) = client.try_create(create_room).await {
            assert_eq!(err.code, ErrorCode::ElementIdMismatch as u32)
        } else {
            panic!("should err")
        }
    }
}

mod member {

    use super::*;

    #[actix_rt::test]
    #[named]
    async fn member() {
        let mut client = ControlClient::new().await;
        client.create(create_room_req(test_name!())).await;

        let add_member = MemberBuilder::default()
            .id("test-member")
            .credentials("qwerty")
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

        let sids = client.create(add_member).await;
        let e2e_test_member_sid =
            sids.get(&"test-member".to_string()).unwrap().as_str();
        assert_eq!(
            e2e_test_member_sid,
            format!(
                "ws://127.0.0.1:8080/ws/{}/test-member/qwerty",
                test_name!()
            )
        );

        let member = client.get(&format!("{}/test-member", test_name!())).await;
        let member = take_member(member);
        assert_eq!(member.pipeline.len(), 1);
        assert_eq!(member.credentials.as_str(), "qwerty");
    }

    #[actix_rt::test]
    #[named]
    async fn cant_create_member_in_non_existent_room() {
        let mut client = ControlClient::new().await;

        let create_member = MemberBuilder::default()
            .id("caller")
            .build()
            .unwrap()
            .build_request(test_name!());

        if let Err(err) = client.try_create(create_member).await {
            assert_eq!(err.code, ErrorCode::RoomNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[actix_rt::test]
    #[named]
    async fn cant_create_members_with_duplicate_ids() {
        let mut client = ControlClient::new().await;

        let create_room = RoomBuilder::default()
            .id(test_name!())
            .build()
            .unwrap()
            .build_request("");

        client.create(create_room).await;

        let create_member = MemberBuilder::default()
            .id("caller")
            .build()
            .unwrap()
            .build_request(test_name!());

        client.create(create_member.clone()).await;

        if let Err(err) = client.try_create(create_member).await {
            assert_eq!(err.code, ErrorCode::MemberAlreadyExists as u32)
        } else {
            panic!("should err")
        }
    }

    #[actix_rt::test]
    async fn element_id_mismatch() {
        let mut client = ControlClient::new().await;

        let create_member = MemberBuilder::default()
            .id("asd")
            .build()
            .unwrap()
            .build_request("qwe/qwe");

        if let Err(err) = client.try_create(create_member).await {
            assert_eq!(err.code, ErrorCode::ElementIdMismatch as u32)
        } else {
            panic!("should err")
        }
    }
}

mod endpoint {
    use std::time::Duration;

    use futures::{channel::mpsc, StreamExt as _};
    use medea_client_api_proto::{Event, PeerUpdate};
    use tokio::time::timeout;

    use super::*;

    #[actix_rt::test]
    #[named]
    async fn endpoint() {
        let mut client = ControlClient::new().await;
        client.create(create_room_req(test_name!())).await;

        let create_req = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Never)
            .build()
            .unwrap()
            .build_request(format!("{}/responder", test_name!()));

        let sids = client.create(create_req).await;
        assert_eq!(sids.len(), 0);

        let endpoint = client
            .get(&format!("{}/responder/publish", test_name!()))
            .await;
        let endpoint = take_webrtc_pub(endpoint);
        assert_eq!(
            endpoint.p2p,
            proto::web_rtc_publish_endpoint::P2p::Never as i32
        );
    }

    #[actix_rt::test]
    #[named]
    async fn cant_create_endpoint_in_non_existent_member() {
        let mut client = ControlClient::new().await;

        let create_room = RoomBuilder::default()
            .id(test_name!())
            .build()
            .unwrap()
            .build_request("");

        client.create(create_room).await;

        let create_play = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
            .build()
            .unwrap()
            .build_request(format!("{}/member", test_name!()));

        if let Err(err) = client.try_create(create_play).await {
            assert_eq!(err.code, ErrorCode::MemberNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[actix_rt::test]
    #[named]
    async fn cant_create_endpoint_in_non_existent_room() {
        let mut client = ControlClient::new().await;

        let create_publish = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
            .build()
            .unwrap()
            .build_request(format!("{}/member", test_name!()));

        if let Err(err) = client.try_create(create_publish).await {
            assert_eq!(err.code, ErrorCode::RoomNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[actix_rt::test]
    #[named]
    async fn cant_create_endpoints_with_duplicate_ids() {
        let mut client = ControlClient::new().await;

        let create_room = RoomBuilder::default()
            .id(test_name!())
            .add_member(MemberBuilder::default().id("member").build().unwrap())
            .build()
            .unwrap()
            .build_request("");

        client.create(create_room).await;

        let create_endpoint = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
            .build()
            .unwrap()
            .build_request(format!("{}/member", test_name!()));

        client.create(create_endpoint.clone()).await;

        if let Err(err) = client.try_create(create_endpoint).await {
            assert_eq!(err.code, ErrorCode::EndpointAlreadyExists as u32)
        } else {
            panic!("should err")
        }
    }

    #[actix_rt::test]
    #[named]
    async fn cant_create_play_endpoint_when_no_pusblish_endpoints() {
        let mut client = ControlClient::new().await;

        let create_room = RoomBuilder::default()
            .id(test_name!())
            .add_member(MemberBuilder::default().id("member").build().unwrap())
            .build()
            .unwrap()
            .build_request("");

        client.create(create_room).await;

        let create_endpoint = WebRtcPlayEndpointBuilder::default()
            .id("play")
            .src(format!("local://{}/member/publish", test_name!()))
            .build()
            .unwrap()
            .build_request(format!("{}/member", test_name!()));

        if let Err(err) = client.try_create(create_endpoint).await {
            assert_eq!(err.code, ErrorCode::EndpointNotFound as u32)
        } else {
            panic!("should err")
        }
    }

    #[actix_rt::test]
    async fn element_id_mismatch() {
        let mut client = ControlClient::new().await;

        let create_endpoint = WebRtcPublishEndpointBuilder::default()
            .id("asd")
            .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
            .build()
            .unwrap()
            .build_request("qwe");

        if let Err(err) = client.try_create(create_endpoint).await {
            assert_eq!(err.code, ErrorCode::ElementIdMismatch as u32)
        } else {
            panic!("should err")
        }
    }

    /// Checks that all needed [`Event`]s are sent when Control API adds
    /// `Endpoint` to the already interconnected `Member`s.
    #[actix_rt::test]
    #[named]
    async fn create_endpoint_in_the_interconnected_members() {
        let mut client = ControlClient::new().await;
        let credentials = client.create(create_room_req(test_name!())).await;

        let (publisher_tx, mut rx) = mpsc::unbounded::<()>();
        let publisher_done = timeout(Duration::from_secs(5), rx.next());
        let (responder_tx, mut rx) = mpsc::unbounded::<()>();
        let responder_done = timeout(Duration::from_secs(5), rx.next());
        let (negotiation_finished_tx, mut rx) = mpsc::unbounded::<()>();
        let negotiation_finished = timeout(Duration::from_secs(5), rx.next());

        let _publisher = TestMember::connect(
            credentials.get("publisher").unwrap(),
            Some(Box::new(move |event, _, _| {
                match event {
                    Event::PeerUpdated { updates, .. } => {
                        if updates
                            .iter()
                            .any(|u| enum_eq!(PeerUpdate::Added, u))
                        {
                            publisher_tx.unbounded_send(()).unwrap();
                        }
                    }
                    Event::SdpAnswerMade { .. } => {
                        negotiation_finished_tx.unbounded_send(()).unwrap();
                    }
                    _ => (),
                };
            })),
            None,
            None,
            true,
        )
        .await;
        let _responder = TestMember::connect(
            credentials.get("responder").unwrap(),
            Some(Box::new(move |event, _, events| {
                if let Event::PeerUpdated { updates, .. } = event {
                    if updates.iter().any(|u| enum_eq!(PeerUpdate::Added, u)) {
                        responder_tx.unbounded_send(()).unwrap();
                        let sdp_answer_made_count = events
                            .iter()
                            .filter(|e| {
                                if let Event::SdpAnswerMade { .. } = e {
                                    true
                                } else {
                                    false
                                }
                            })
                            .count();
                        if sdp_answer_made_count == 2 {
                            responder_tx.unbounded_send(()).unwrap();
                        }
                    }
                };
            })),
            None,
            None,
            true,
        )
        .await;

        negotiation_finished.await.unwrap();

        let create_publish_endpoint = WebRtcPublishEndpointBuilder::default()
            .id("publish")
            .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
            .build()
            .unwrap()
            .build_request(format!("{}/responder", test_name!()));
        client.create(create_publish_endpoint).await;

        let create_play_endpoint = WebRtcPlayEndpointBuilder::default()
            .id("play-receiver")
            .src(format!("local://{}/responder/publish", test_name!()))
            .build()
            .unwrap()
            .build_request(format!("{}/publisher", test_name!()));
        client.create(create_play_endpoint).await;

        let (publisher_res, responder_res) =
            futures::future::join(publisher_done, responder_done).await;
        publisher_res.unwrap().unwrap();
        responder_res.unwrap().unwrap();
    }
}
