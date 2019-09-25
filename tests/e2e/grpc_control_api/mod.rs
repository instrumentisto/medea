mod create;
mod delete;

use std::{collections::HashMap, sync::Arc};

use grpcio::{ChannelBuilder, EnvBuilder};
use medea_control_api_proto::grpc::{
    control_api::{
        CreateRequest, Element, Error, IdRequest, Member, Member_Element, Room,
        Room_Element, WebRtcPlayEndpoint, WebRtcPublishEndpoint,
        WebRtcPublishEndpoint_P2P,
    },
    control_api_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

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

    pub fn create(&self, req: &CreateRequest) -> HashMap<String, String> {
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
