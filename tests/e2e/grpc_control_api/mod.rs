use std::{collections::HashMap, sync::Arc};

use actix_web::web::delete;
use grpcio::{ChannelBuilder, EnvBuilder};
use medea::api::error_codes::ErrorCode as MedeaErrorCode;
use medea_control_api_proto::grpc::{
    control_api::{
        ApplyRequest, CreateRequest, CreateResponse, Element, Error,
        GetResponse, IdRequest, Member, Member_Element, Room, Room_Element,
        WebRtcPlayEndpoint, WebRtcPublishEndpoint, WebRtcPublishEndpoint_P2P,
    },
    control_api_grpc::ControlApiClient,
};
use protobuf::RepeatedField;
use serde_json::error::ErrorCode::ControlCharacterWhileParsingString;

struct ControlClient(ControlApiClient);

impl ControlClient {
    pub fn new() -> Self {
        let env = Arc::new(EnvBuilder::new().build());
        let ch = ChannelBuilder::new(env).connect("localhost:6565");
        ControlClient(ControlApiClient::new(ch))
    }

    pub fn get(&self, uri: &str) -> Element {
        let mut get_room_request = IdRequest::new();
        let mut room = RepeatedField::new();
        room.push(uri.to_string());
        get_room_request.set_id(room);

        let mut resp = self.0.get(&get_room_request).expect("get room");
        if resp.has_error() {
            panic!("{:?}", resp.get_error());
        }
        resp.take_elements().remove(&uri.to_string()).unwrap()
    }

    pub fn try_get(&self, uri: &str) -> Result<Element, Error> {
        let mut get_room_request = IdRequest::new();
        let mut room = RepeatedField::new();
        room.push(uri.to_string());
        get_room_request.set_id(room);

        let mut resp = self.0.get(&get_room_request).expect("get room");
        if resp.has_error() {
            return Err(resp.take_error());
        }
        Ok(resp.take_elements().remove(&uri.to_string()).unwrap())
    }

    pub fn create(&self, req: CreateRequest) -> HashMap<String, String> {
        let resp = self.0.create(&req).expect("create endpoint");
        if resp.has_error() {
            panic!("{:?}", resp.get_error());
        }

        resp.sid
    }

    pub fn delete(&self, ids: &[&str]) {
        let mut delete_req = IdRequest::new();
        let mut delete_ids = RepeatedField::new();
        ids.into_iter()
            .for_each(|id| delete_ids.push(id.to_string()));
        delete_req.set_id(delete_ids);

        let resp = self.0.delete(&delete_req).unwrap();
        if resp.has_error() {
            panic!("{:?}", resp.get_error());
        }
    }
}

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
fn create_room() {
    let create_req = create_room_req("create-room-e2e-test");

    let client = ControlClient::new();
    let resp = client.create(create_req);

    let mut get_resp = client.get("local://create-room-e2e-test");
    let room = get_resp.take_room();

    let responder = room
        .get_pipeline()
        .get(&"local://create-room-e2e-test/responder".to_string())
        .unwrap()
        .get_member();
    assert_eq!(responder.get_credentials(), "test");
    let responder_pipeline = responder.get_pipeline();
    assert_eq!(responder_pipeline.len(), 1);
    let responder_play = responder_pipeline
        .get(&"local://create-room-e2e-test/responder/play".to_string())
        .unwrap()
        .get_webrtc_play();
    assert_eq!(
        responder_play.get_src(),
        "local://create-room-e2e-test/publisher/publish"
    );

    let publisher = room
        .get_pipeline()
        .get(&"local://create-room-e2e-test/publisher".to_string())
        .unwrap()
        .get_member();
    assert_ne!(publisher.get_credentials(), "test");
    assert_ne!(publisher.get_credentials(), "");
    let publisher_pipeline = responder.get_pipeline();
    assert_eq!(publisher_pipeline.len(), 1);
}

#[test]
fn create_member() {
    let client = ControlClient::new();
    client.create(create_room_req("create-member-e2e-test"));

    let create_req = {
        let mut create_member_request = CreateRequest::new();
        let mut member = Member::new();
        let mut member_pipeline = HashMap::new();

        let mut play_endpoint = WebRtcPlayEndpoint::new();
        play_endpoint.set_src(
            "local://create-member-e2e-test/publisher/publish".to_string(),
        );
        let mut member_element = Member_Element::new();
        member_element.set_webrtc_play(play_endpoint);
        member_pipeline.insert("play".to_string(), member_element);

        member.set_credentials("qwerty".to_string());
        member.set_pipeline(member_pipeline);
        create_member_request.set_id(
            "local://create-member-e2e-test/e2e-test-member".to_string(),
        );
        create_member_request.set_member(member);

        create_member_request
    };

    let sids = client.create(create_req);
    let e2e_test_member_sid =
        sids.get(&"e2e-test-member".to_string()).unwrap().as_str();
    assert_eq!(
        e2e_test_member_sid,
        "ws://0.0.0.0:8080/create-member-e2e-test/e2e-test-member/qwerty"
    );

    let member = client
        .get("local://create-member-e2e-test/e2e-test-member")
        .take_member();
    assert_eq!(member.get_pipeline().len(), 1);
    assert_eq!(member.get_credentials(), "qwerty");
}

#[test]
fn create_endpoint() {
    let client = ControlClient::new();
    client.create(create_room_req("create-endpoint-e2e-test"));

    let create_req = {
        let mut create_endpoint_request = CreateRequest::new();
        let mut endpoint = WebRtcPublishEndpoint::new();
        endpoint.set_p2p(WebRtcPublishEndpoint_P2P::NEVER);
        create_endpoint_request.set_id(
            "local://create-endpoint-e2e-test/responder/publish".to_string(),
        );
        create_endpoint_request.set_webrtc_pub(endpoint);

        create_endpoint_request
    };
    let sids = client.create(create_req);
    assert_eq!(sids.len(), 0);

    let endpoint = client
        .get("local://create-endpoint-e2e-test/responder/publish")
        .take_webrtc_pub();
    assert_eq!(endpoint.get_p2p(), WebRtcPublishEndpoint_P2P::NEVER);
}

#[test]
fn delete_room() {
    let client = ControlClient::new();
    client.create(create_room_req("delete-room-e2e-test"));
    client.delete(&["local://delete-room-e2e-test"]);

    let get_room_err = match client.try_get("local://delete-room-e2e-test") {
        Ok(_) => panic!("Room not deleted!"),
        Err(e) => e,
    };
    assert_eq!(get_room_err.code, MedeaErrorCode::RoomNotFound as u32);
}

#[test]
fn delete_member() {
    let client = ControlClient::new();
    client.create(create_room_req("delete-member-e2e-test"));
    client.delete(&["local://delete-member-e2e-test/publisher"]);

    let get_member_err =
        match client.try_get("local://delete-member-e2e-test/publisher") {
            Ok(_) => panic!("Member not deleted!"),
            Err(e) => e,
        };
    assert_eq!(get_member_err.code, MedeaErrorCode::MemberNotFound as u32);
}

#[test]
fn delete_endpoint() {
    let client = ControlClient::new();
    client.create(create_room_req("delete-endpoint-e2e-test"));
    client.delete(&["local://delete-endpoint-e2e-test/publisher/publish"]);

    let get_endpoint_err = match client
        .try_get("local://delete-endpoint-e2e-test/publisher/publish")
    {
        Ok(_) => panic!("Endpoint not deleted!"),
        Err(e) => e,
    };
    assert_eq!(
        get_endpoint_err.code,
        MedeaErrorCode::EndpointNotFound as u32
    );
}
