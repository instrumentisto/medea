//! Tests for gRPC [Medea]'s [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

mod create;
mod delete;
mod signaling;

use std::{collections::HashMap, sync::Arc};

use derive_builder::*;
use medea::conf::ControlApi;
use medea_control_api_proto::grpc::medea::{
    control_api_client::ControlApiClient,
    create_request::El as CreateRequestEl,
    element::El as RootEl,
    member::{element::El as MemberEl, Element as Member_Element},
    room::{element::El as RoomEl, Element as Room_Element},
    web_rtc_publish_endpoint::P2p as WebRtcPublishEndpoint_P2P,
    CreateRequest, Element, Error, IdRequest, Member as GrpcMember,
    Room as GrpcRoom, WebRtcPlayEndpoint as GrpcWebRtcPlayEndpoint,
    WebRtcPublishEndpoint as GrpcWebRtcPublishEndpoint,
};
use tonic::transport::Channel;

pub struct Elem(pub Element);

impl Elem {
    pub fn take_room(self) -> GrpcRoom {
        match self.0.el.unwrap() {
            RootEl::Room(room) => room,
            _ => panic!("Not Room element!"),
        }
    }

    pub fn take_member(self) -> GrpcMember {
        match self.0.el.unwrap() {
            RootEl::Member(member) => member,
            _ => panic!("Not Room element!"),
        }
    }

    pub fn take_webrtc_pub(self) -> GrpcWebRtcPublishEndpoint {
        match self.0.el.unwrap() {
            RootEl::WebrtcPub(webrtc_pub) => webrtc_pub,
            _ => panic!("Not Room element!"),
        }
    }

    pub fn take_webrtc_play(self) -> GrpcWebRtcPlayEndpoint {
        match self.0.el.unwrap() {
            RootEl::WebrtcPlay(webrtc_play) => webrtc_play,
            _ => panic!("Not Room element!"),
        }
    }
}

/// Client for [Medea]'s gRPC [Control API].
///
/// [Medea]: https://github.com/instrumentisto/medea
#[derive(Clone)]
pub struct ControlClient(ControlApiClient<Channel>);

impl ControlClient {
    /// Create new [`ControlClient`].
    ///
    /// Client will connect to `localhost:6565`.
    ///
    /// Note that this function don't connects to the server. This mean that
    /// when you call [`ControlClient::new`] and server not working you will
    /// don't know it until try to send something with this client.
    pub async fn new() -> Self {
        Self(
            ControlApiClient::connect("http://127.0.0.1:6565")
                .await
                .unwrap(),
        )
    }

    /// Gets some [`Element`] by local URI.
    ///
    /// # Panics
    ///
    /// - if [`GetResponse`] has error
    /// - if connection with server failed
    pub async fn get(&mut self, uri: &str) -> Elem {
        let room = vec![uri.to_string()];
        let get_room_request = IdRequest { fid: room };

        let mut resp = self.0.get(get_room_request).await.unwrap().into_inner();
        if let Some(err) = resp.error {
            panic!("{:?}", err);
        }

        Elem(resp.elements.remove(&uri.to_string()).unwrap())
    }

    /// Tries to get some [`Element`] by local URI.
    ///
    /// # Panics
    ///
    /// - if connection with server failed.
    pub async fn try_get(&mut self, uri: &str) -> Result<Element, Error> {
        let room = vec![uri.to_string()];
        let get_room_request = IdRequest { fid: room };

        let mut resp = self.0.get(get_room_request).await.unwrap().into_inner();
        if let Some(e) = resp.error {
            return Err(e);
        }

        Ok(resp.elements.remove(&uri.to_string()).unwrap())
    }

    /// Creates `Element` and returns it sids.
    ///
    /// # Panics
    ///
    /// - if [`CreateResponse`] has error.
    /// - if connection with server failed.
    pub async fn create(
        &mut self,
        req: CreateRequest,
    ) -> HashMap<String, String> {
        let resp = self.0.create(req).await.unwrap().into_inner();
        if let Some(e) = resp.error {
            panic!("{:?}", e);
        }

        resp.sid
    }

    /// Tries to create `Element` and returns it sids.
    ///
    /// # Panics
    ///
    /// - if connection with server failed.
    pub async fn try_create(
        &mut self,
        req: CreateRequest,
    ) -> Result<HashMap<String, String>, Error> {
        let mut resp = self.0.create(req).await.unwrap().into_inner();

        if let Some(e) = resp.error {
            Err(e)
        } else {
            Ok(resp.sid)
        }
    }

    /// Deletes `Element`s by local URIs.
    ///
    /// # Panics
    ///
    /// - if [`Response`] has error
    /// - if connection with server failed.
    pub async fn delete(&mut self, ids: &[&str]) -> Result<(), Error> {
        let delete_ids = ids.iter().map(|id| id.to_string()).collect();
        let delete_req = IdRequest { fid: delete_ids };

        let mut resp = self.0.delete(delete_req).await.unwrap().into_inner();
        if let Some(e) = resp.error {
            Err(e)
        } else {
            Ok(())
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

impl Room {
    pub fn build_request<T: Into<String>>(self, uri: T) -> CreateRequest {
        let members = self
            .members
            .into_iter()
            .map(|(id, member)| {
                let room_element = Room_Element {
                    el: Some(RoomEl::Member(member.into())),
                };

                (id, room_element)
            })
            .collect();
        let grpc_room = GrpcRoom {
            id: self.id,
            pipeline: members,
        };

        CreateRequest {
            parent_fid: uri.into(),
            el: Some(CreateRequestEl::Room(grpc_room)),
        }
    }
}

impl RoomBuilder {
    pub fn add_member<T: Into<Member>>(&mut self, member: T) -> &mut Self {
        let member = member.into();

        self.members
            .get_or_insert(HashMap::new())
            .insert(member.id.clone(), member);

        self
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
    #[builder(default = "None")]
    #[builder(setter(strip_option))]
    on_join: Option<String>,
    #[builder(default = "None")]
    #[builder(setter(strip_option))]
    on_leave: Option<String>,
}

impl Into<GrpcMember> for Member {
    fn into(self) -> GrpcMember {
        let pipeline = self
            .endpoints
            .into_iter()
            .map(|(id, element)| (id, element.into()))
            .collect();

        GrpcMember {
            id: self.id,
            pipeline,
            on_leave: self.on_leave.unwrap_or_default(),
            on_join: self.on_join.unwrap_or_default(),
            credentials: self.credentials.unwrap_or_default(),
        }
    }
}

impl Member {
    fn build_request<T: Into<String>>(self, url: T) -> CreateRequest {
        CreateRequest {
            parent_fid: url.into(),
            el: Some(CreateRequestEl::Member(self.into())),
        }
    }
}

impl MemberBuilder {
    pub fn add_endpoint<T: Into<Endpoint>>(&mut self, element: T) -> &mut Self {
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
        let member_el = match self {
            Self::WebRtcPlayElement(element) => {
                MemberEl::WebrtcPlay(element.into())
            }
            Self::WebRtcPublishElement(element) => {
                MemberEl::WebrtcPub(element.into())
            }
        };

        Member_Element {
            el: Some(member_el),
        }
    }
}

#[derive(Builder, Clone)]
#[builder(setter(into))]
pub struct WebRtcPlayEndpoint {
    id: String,
    src: String,
}

impl WebRtcPlayEndpoint {
    pub fn build_request<T: Into<String>>(self, url: T) -> CreateRequest {
        CreateRequest {
            el: Some(CreateRequestEl::WebrtcPlay(self.into())),
            parent_fid: url.into(),
        }
    }
}

impl Into<GrpcWebRtcPlayEndpoint> for WebRtcPlayEndpoint {
    fn into(self) -> GrpcWebRtcPlayEndpoint {
        GrpcWebRtcPlayEndpoint {
            src: self.src,
            on_start: String::new(),
            on_stop: String::new(),
            id: self.id,
            force_relay: false,
        }
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
    pub fn build_request<T: Into<String>>(self, url: T) -> CreateRequest {
        CreateRequest {
            el: Some(CreateRequestEl::WebrtcPub(self.into())),
            parent_fid: url.into(),
        }
    }
}

impl Into<GrpcWebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn into(self) -> GrpcWebRtcPublishEndpoint {
        GrpcWebRtcPublishEndpoint {
            p2p: self.p2p_mode as i32,
            on_start: String::default(),
            on_stop: String::default(),
            id: self.id,
            force_relay: bool::default(),
        }
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
        .id(room_id.to_string())
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(WebRtcPublishEndpoint_P2P::Always)
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
        .build_request(String::new())
}
