/// Tests for gRPC [Medea]'s [Control API].
///
/// [Medea]: https://github.com/instrumentisto/medea
/// [Control API]: https://tinyurl.com/yxsqplq7
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

/// Client for [Medea]'s gRPC [Control API].
///
/// [Medea]: https://github.com/instrumentisto/medea
#[derive(Clone)]
pub struct ControlClient(ControlApiClient);

impl ControlClient {
    /// Create new [`ControlClient`].
    ///
    /// Client will connect to `localhost:6565`.
    ///
    /// Note that this function don't connects to the server. This mean that
    /// when you call [`ControlClient::new`] and server not working you will
    /// don't know it until try to send something with this client.
    pub fn new() -> Self {
        let env = Arc::new(EnvBuilder::new().build());
        let ch = ChannelBuilder::new(env).connect("localhost:6565");
        ControlClient(ControlApiClient::new(ch))
    }

    /// Gets some [`Element`] by local URI.
    ///
    /// # Panics
    ///
    /// - if [`GetResponse`] has error
    /// - if connection with server failed
    pub fn get(&self, uri: &str) -> Element {
        let mut get_room_request = IdRequest::new();
        let mut room = RepeatedField::new();
        room.push(uri.to_string());
        get_room_request.set_id(room);

        let mut resp = self.0.get(&get_room_request).unwrap();
        if resp.has_error() {
            panic!("{:?}", resp.get_error());
        }
        resp.take_elements().remove(&uri.to_string()).unwrap()
    }

    /// Tries to get some [`Element`] by local URI.
    ///
    /// # Panics
    ///
    /// - if connection with server failed.
    pub fn try_get(&self, uri: &str) -> Result<Element, Error> {
        let mut get_room_request = IdRequest::new();
        let mut room = RepeatedField::new();
        room.push(uri.to_string());
        get_room_request.set_id(room);

        let mut resp = self.0.get(&get_room_request).unwrap();
        if resp.has_error() {
            return Err(resp.take_error());
        }
        Ok(resp.take_elements().remove(&uri.to_string()).unwrap())
    }

    /// Creates `Element` and returns it sids.
    ///
    /// # Panics
    ///
    /// - if [`CreateResponse`] has error.
    /// - if connection with server failed.
    pub fn create(&self, req: &CreateRequest) -> HashMap<String, String> {
        let resp = self.0.create(&req).unwrap();
        if resp.has_error() {
            panic!("{:?}", resp.get_error());
        }

        resp.sid
    }

    /// Deletes `Element`s by local URIs.
    ///
    /// # Panics
    ///
    /// - if [`Response`] has error
    /// - if connection with server failed.
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

/// Creates [`CreateRequest`] for creating `Room` element with provided room ID.
///
/// # Spec of `Room` which will be created with this [`CreateRequest`]
///
/// ```yaml
/// kind: Room
///   id: {{ room_id }}
///   spec:
///     pipeline:
///       publisher:
///         kind: Member
///         spec:
///           pipeline:
///             publish:
///               kind: WebRtcPublishEndpoint
///               spec:
///                 p2p: Always
///       responder:
///         kind: Member
///         credentials: test
///         spec:
///           pipeline:
///             play:
///               kind: WebRtcPlayEndpoint
///               spec:
///                 src: "local://{{ room_id }}/publisher/publish"
/// ```
pub fn create_room_req(room_id: &str) -> CreateRequest {
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
