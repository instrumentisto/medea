//! Tests for gRPC [Medea]'s [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

mod create;
mod credentials;
mod delete;
mod rpc_settings;
mod signaling;

use std::{collections::HashMap, time::Duration};

use derive_builder::*;
use medea_control_api_proto::grpc::api::{
    self as proto, control_api_client::ControlApiClient, member::Credentials,
};
use tonic::transport::Channel;

macro_rules! gen_elem_take_fn {
    ($name:tt -> $variant:tt($output:ty)) => {
        pub fn $name(el: proto::Element) -> $output {
            match el.el.unwrap() {
                proto::element::El::$variant(elem) => elem,
                _ => panic!("Not {} element!", stringify!($variant)),
            }
        }
    };
}

gen_elem_take_fn!(take_room -> Room(proto::Room));

gen_elem_take_fn!(take_member -> Member(proto::Member));

gen_elem_take_fn!(take_webrtc_pub -> WebrtcPub(proto::WebRtcPublishEndpoint));

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

    /// Gets some [`proto::Element`] by local URI.
    ///
    /// # Panics
    ///
    /// - if [`GetResponse`] has error
    /// - if connection with server failed
    pub async fn get(&mut self, uri: &str) -> proto::Element {
        let room = vec![uri.to_string()];
        let get_room_request = proto::IdRequest { fid: room };

        let mut resp = self.0.get(get_room_request).await.unwrap().into_inner();
        if let Some(err) = resp.error {
            panic!("{:?}", err);
        }
        resp.elements.remove(&uri.to_string()).unwrap()
    }

    /// Tries to get some [`proto::Element`] by local URI.
    ///
    /// # Panics
    ///
    /// - if connection with server failed.
    pub async fn try_get(
        &mut self,
        uri: &str,
    ) -> Result<proto::Element, proto::Error> {
        let room = vec![uri.to_string()];
        let get_room_request = proto::IdRequest { fid: room };

        let mut resp = self.0.get(get_room_request).await.unwrap().into_inner();
        if let Some(e) = resp.error {
            return Err(e);
        }
        Ok(resp.elements.remove(&uri.to_string()).unwrap())
    }

    /// Creates `proto::Element` and returns it sids.
    ///
    /// # Panics
    ///
    /// - if [`CreateResponse`] has error.
    /// - if connection with server failed.
    pub async fn create(
        &mut self,
        req: proto::CreateRequest,
    ) -> HashMap<String, String> {
        let resp = self.0.create(req).await.unwrap().into_inner();
        if let Some(e) = resp.error {
            panic!("{:?}", e);
        }

        resp.sid
    }

    /// Tries to create `proto::Element` and returns it sids.
    ///
    /// # Panics
    ///
    /// - if connection with server failed.
    pub async fn try_create(
        &mut self,
        req: proto::CreateRequest,
    ) -> Result<HashMap<String, String>, proto::Error> {
        let resp = self.0.create(req).await.unwrap().into_inner();

        if let Some(e) = resp.error {
            Err(e)
        } else {
            Ok(resp.sid)
        }
    }

    /// Deletes `proto::Element`s by local URIs.
    ///
    /// # Panics
    ///
    /// - if [`Response`] has error
    /// - if connection with server failed.
    pub async fn delete(&mut self, ids: &[&str]) -> Result<(), proto::Error> {
        let delete_ids = ids.iter().map(|id| (*id).to_string()).collect();
        let delete_req = proto::IdRequest { fid: delete_ids };

        let resp = self.0.delete(delete_req).await.unwrap().into_inner();
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
    pub fn build_request<T: Into<String>>(
        self,
        uri: T,
    ) -> proto::CreateRequest {
        let members = self
            .members
            .into_iter()
            .map(|(id, member)| {
                let room_element = proto::room::Element {
                    el: Some(proto::room::element::El::Member(member.into())),
                };

                (id, room_element)
            })
            .collect();
        let grpc_room = proto::Room {
            id: self.id,
            pipeline: members,
        };

        proto::CreateRequest {
            parent_fid: uri.into(),
            el: Some(proto::create_request::El::Room(grpc_room)),
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
    credentials: Option<proto::member::Credentials>,
    #[builder(default = "HashMap::new()")]
    endpoints: HashMap<String, Endpoint>,
    #[builder(default = "None")]
    #[builder(setter(strip_option))]
    on_join: Option<String>,
    #[builder(default = "None")]
    #[builder(setter(strip_option))]
    on_leave: Option<String>,
    #[builder(default = "None")]
    ping_interval: Option<Duration>,
    #[builder(default = "None")]
    idle_timeout: Option<Duration>,
    #[builder(default = "None")]
    reconnect_timeout: Option<Duration>,
}

impl Into<proto::Member> for Member {
    fn into(self) -> proto::Member {
        let pipeline = self
            .endpoints
            .into_iter()
            .map(|(id, element)| (id, element.into()))
            .collect();

        proto::Member {
            id: self.id,
            pipeline,
            on_leave: self.on_leave.unwrap_or_default(),
            on_join: self.on_join.unwrap_or_default(),
            credentials: self.credentials,
            ping_interval: self.ping_interval.map(Into::into),
            idle_timeout: self.idle_timeout.map(Into::into),
            reconnect_timeout: self.reconnect_timeout.map(Into::into),
        }
    }
}

impl Member {
    fn build_request<T: Into<String>>(self, url: T) -> proto::CreateRequest {
        proto::CreateRequest {
            parent_fid: url.into(),
            el: Some(proto::create_request::El::Member(self.into())),
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

impl Into<proto::member::Element> for Endpoint {
    fn into(self) -> proto::member::Element {
        let member_el = match self {
            Self::WebRtcPlayElement(element) => {
                proto::member::element::El::WebrtcPlay(element.into())
            }
            Self::WebRtcPublishElement(element) => {
                proto::member::element::El::WebrtcPub(element.into())
            }
        };

        proto::member::Element {
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
    pub fn build_request<T: Into<String>>(
        self,
        url: T,
    ) -> proto::CreateRequest {
        proto::CreateRequest {
            el: Some(proto::create_request::El::WebrtcPlay(self.into())),
            parent_fid: url.into(),
        }
    }
}

impl Into<proto::WebRtcPlayEndpoint> for WebRtcPlayEndpoint {
    fn into(self) -> proto::WebRtcPlayEndpoint {
        proto::WebRtcPlayEndpoint {
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
    p2p_mode: proto::web_rtc_publish_endpoint::P2p,
}

impl WebRtcPublishEndpoint {
    pub fn build_request<T: Into<String>>(
        self,
        url: T,
    ) -> proto::CreateRequest {
        proto::CreateRequest {
            el: Some(proto::create_request::El::WebrtcPub(self.into())),
            parent_fid: url.into(),
        }
    }
}

impl Into<proto::WebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn into(self) -> proto::WebRtcPublishEndpoint {
        use proto::web_rtc_publish_endpoint::{
            AudioSettings, PublishPolicy, VideoSettings,
        };
        proto::WebRtcPublishEndpoint {
            p2p: self.p2p_mode as i32,
            on_start: String::default(),
            on_stop: String::default(),
            id: self.id,
            force_relay: bool::default(),
            audio_settings: Some(AudioSettings {
                publish_policy: PublishPolicy::Optional as i32,
            }),
            video_settings: Some(VideoSettings {
                publish_policy: PublishPolicy::Optional as i32,
            }),
        }
    }
}

impl Into<Endpoint> for WebRtcPublishEndpoint {
    fn into(self) -> Endpoint {
        Endpoint::WebRtcPublishElement(self)
    }
}

/// Creates [`proto::CreateRequest`] for creating `Room` element with provided
/// `Room` ID.
///
/// # Spec of `Room` which will be created with this [`proto::CreateRequest`]
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
///       credentials:
///         plain: test
///       spec:
///         pipeline:
///           play:
///             kind: WebRtcPlayEndpoint
///             spec:
///               src: "local://{{ room_id }}/publisher/publish"
/// ```
pub fn create_room_req(room_id: &str) -> proto::CreateRequest {
    RoomBuilder::default()
        .id(room_id.to_string())
        .add_member(
            MemberBuilder::default()
                .id("publisher")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .add_member(
            MemberBuilder::default()
                .id("responder")
                .credentials(Credentials::Plain(String::from("test")))
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

/// Creates [`proto::CreateRequest`] for creating `Room` element with provided
/// `Room` ID.
///
/// # Spec of `Room` which will be created with this [`proto::CreateRequest`]
///
/// ```yaml
/// kind: Room
/// id: {{ room_id }}
/// spec:
///   pipeline:
///     alice:
///       kind: Member
///       spec:
///         pipeline:
///           publish:
///             kind: WebRtcPublishEndpoint
///             spec:
///               p2p: Always
///           play:
///             kind: WebRtcPlayEndpoint
///             spec:
///               src: "local://{{ room_id }}/bob/publish"
///     bob:
///       kind: Member
///       credentials:
///         plain: test
///       spec:
///         pipeline:
///           play:
///             kind: WebRtcPlayEndpoint
///             spec:
///               src: "local://{{ room_id }}/alice/publish"
/// ```
pub fn pub_pub_room_req(room_id: &str) -> proto::CreateRequest {
    RoomBuilder::default()
        .id(room_id.to_string())
        .add_member(
            MemberBuilder::default()
                .id("alice")
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                        .build()
                        .unwrap(),
                )
                .add_endpoint(
                    WebRtcPlayEndpointBuilder::default()
                        .id("play")
                        .src(format!("local://{}/bob/publish", room_id))
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        )
        .add_member(
            MemberBuilder::default()
                .id("bob")
                .credentials(Credentials::Plain(String::from("test")))
                .add_endpoint(
                    WebRtcPublishEndpointBuilder::default()
                        .id("publish")
                        .p2p_mode(proto::web_rtc_publish_endpoint::P2p::Always)
                        .build()
                        .unwrap(),
                )
                .add_endpoint(
                    WebRtcPlayEndpointBuilder::default()
                        .id("play")
                        .src(format!("local://{}/alice/publish", room_id))
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
