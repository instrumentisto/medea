//! Tests for gRPC [Medea]'s [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

mod create;
mod delete;
mod signaling;

use std::{collections::HashMap, sync::Arc};

use derive_builder::*;
use grpcio::{ChannelBuilder, EnvBuilder};
use medea_control_api_proto::grpc::{
    api::{
        CreateRequest, Element, Error, IdRequest, Member as GrpcMember,
        Member_Element, Room as GrpcRoom, Room_Element,
        WebRtcPlayEndpoint as GrpcWebRtcPlayEndpoint,
        WebRtcPublishEndpoint as GrpcWebRtcPublishEndpoint,
        WebRtcPublishEndpoint_P2P,
    },
    api_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

/// Client for [Medea]'s gRPC [Control API].
///
/// [Medea]: https://github.com/instrumentisto/medea
#[derive(Clone)]
struct ControlClient(ControlApiClient);

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
        let ch = ChannelBuilder::new(env).connect("127.0.0.1:6565");
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

#[derive(Builder)]
#[builder(setter(into))]
pub struct Room {
    id: String,

    #[builder(default = "HashMap::new()")]
    members: HashMap<String, Member>,
}

impl RoomBuilder {
    fn add_member<T: Into<Member>>(&mut self, member: T) -> &mut Self {
        let member = member.into();

        self.members
            .get_or_insert(HashMap::new())
            .insert(member.id.clone(), member);

        self
    }
}

impl From<Room> for CreateRequest {
    fn from(room: Room) -> Self {
        let mut request = Self::default();

        let mut grpc_room = GrpcRoom::new();
        let mut members = HashMap::new();

        for (id, member) in room.members {
            let mut room_element = Room_Element::new();
            room_element.set_member(member.into());

            members.insert(id, room_element);
        }

        grpc_room.set_pipeline(members);

        request.set_id(room.id);
        request.set_room(grpc_room);

        request
    }
}

#[derive(Builder, Clone)]
#[builder(setter(into))]
pub struct Member {
    id: String,
    #[builder(default = "None")]
    #[builder(setter(strip_option))]
    credentials: Option<String>,
    #[builder(default = "HashMap::new()")]
    endpoints: HashMap<String, Endpoint>,
}

impl Into<GrpcMember> for Member {
    fn into(self) -> GrpcMember {
        let mut grpc_member = GrpcMember::new();

        let mut pipeline = HashMap::new();

        for (id, element) in self.endpoints {
            pipeline.insert(id, element.into());
        }

        if let Some(credentials) = self.credentials {
            grpc_member.set_credentials(credentials)
        }

        grpc_member.set_pipeline(pipeline);

        grpc_member
    }
}

impl Member {
    fn build_request<T: Into<String>>(self, url: T) -> CreateRequest {
        let mut request = CreateRequest::default();

        request.set_id(url.into());
        request.set_member(self.into());

        request
    }
}

impl MemberBuilder {
    fn add_endpoint<T: Into<Endpoint>>(&mut self, element: T) -> &mut Self {
        let element = element.into();

        self.endpoints
            .get_or_insert(HashMap::new())
            .insert(element.id(), element);
        self
    }
}

#[derive(Clone)]
pub enum Endpoint {
    WebRtcPlayElement(WebRtcPlayEndpoint),
    WebRtcPublishElement(WebRtcPublishEndpoint),
}

impl Endpoint {
    fn id(&self) -> String {
        match self {
            Self::WebRtcPlayElement(endpoint) => endpoint.id.clone(),
            Self::WebRtcPublishElement(endpoint) => endpoint.id.clone(),
        }
    }
}

impl Into<Member_Element> for Endpoint {
    fn into(self) -> Member_Element {
        let mut member_elem = Member_Element::new();

        match self {
            Endpoint::WebRtcPlayElement(element) => {
                member_elem.set_webrtc_play(element.into());
            }
            Endpoint::WebRtcPublishElement(element) => {
                member_elem.set_webrtc_pub(element.into())
            }
        }

        member_elem
    }
}

#[derive(Builder, Clone)]
#[builder(setter(into))]
pub struct WebRtcPlayEndpoint {
    id: String,
    src: String,
}

impl WebRtcPlayEndpoint {
    fn _build_request<T: Into<String>>(self, url: T) -> CreateRequest {
        let mut request = CreateRequest::default();

        request.set_id(url.into());
        request.set_webrtc_play(self.into());

        request
    }
}

impl Into<GrpcWebRtcPlayEndpoint> for WebRtcPlayEndpoint {
    fn into(self) -> GrpcWebRtcPlayEndpoint {
        let mut endpoint = GrpcWebRtcPlayEndpoint::new();
        endpoint.set_src(self.src);

        endpoint
    }
}

impl Into<Endpoint> for WebRtcPlayEndpoint {
    fn into(self) -> Endpoint {
        Endpoint::WebRtcPlayElement(self)
    }
}

#[derive(Builder, Clone)]
#[builder(setter(into))]
pub struct WebRtcPublishEndpoint {
    id: String,
    p2p_mode: WebRtcPublishEndpoint_P2P,
}

impl WebRtcPublishEndpoint {
    fn build_request<T: Into<String>>(self, url: T) -> CreateRequest {
        let mut request = CreateRequest::default();

        request.set_id(url.into());
        request.set_webrtc_pub(self.into());

        request
    }
}

impl Into<GrpcWebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn into(self) -> GrpcWebRtcPublishEndpoint {
        let mut endpoint = GrpcWebRtcPublishEndpoint::new();
        endpoint.set_p2p(self.p2p_mode);

        endpoint
    }
}

impl Into<Endpoint> for WebRtcPublishEndpoint {
    fn into(self) -> Endpoint {
        Endpoint::WebRtcPublishElement(self)
    }
}

/// Creates [`CreateRequest`] for creating `Room` element with provided `Room`
/// ID.
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
    RoomBuilder::default()
        .id(format!("local://{}", room_id))
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(WebRtcPublishEndpoint_P2P::ALWAYS)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .add_member(
            MemberBuilder::default()
                .id("responder")
                .credentials("test")
                .add_endpoint(
                    WebRtcPlayEndpointBuilder::default()
                        .id("play")
                        .src(format!("local://{}/publisher/publish", room_id))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
        .into()
}
