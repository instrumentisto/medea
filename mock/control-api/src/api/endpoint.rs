//! `Endpoint` related methods and entities.

use medea_control_api_proto::grpc::api::{
    Member_Element as MemberElementProto,
    Member_Element_oneof_el as MemberElementOneOfEl,
    WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
    WebRtcPublishEndpoint_P2P as P2pModeProto,
};
use serde::{Deserialize, Serialize};

/// P2P mode of [`WebRtcPublishEndpoint`].
#[derive(Debug, Deserialize, Serialize)]
pub enum P2pMode {
    Always,
    Never,
    IfPossible,
}

impl Into<P2pModeProto> for P2pMode {
    fn into(self) -> P2pModeProto {
        match self {
            Self::Always => P2pModeProto::ALWAYS,
            Self::IfPossible => P2pModeProto::IF_POSSIBLE,
            Self::Never => P2pModeProto::NEVER,
        }
    }
}

impl From<P2pModeProto> for P2pMode {
    fn from(proto: P2pModeProto) -> Self {
        match proto {
            P2pModeProto::ALWAYS => Self::Always,
            P2pModeProto::IF_POSSIBLE => Self::IfPossible,
            P2pModeProto::NEVER => Self::Never,
        }
    }
}

/// [Control API]'s `WebRtcPublishEndpoint` representation.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize, Serialize)]
pub struct WebRtcPublishEndpoint {
    /// ID of [`WebRtcPublishEndpoint`].
    #[serde(skip_deserializing)]
    id: String,

    /// Mode of connection for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,
}

impl WebRtcPublishEndpoint {
    /// Converts [`WebRtcPublishEndpoint`] into protobuf
    /// [`WebRtcPublishEndpointProto`].
    pub fn into_proto(self, id: String) -> WebRtcPublishEndpointProto {
        let mut proto = WebRtcPublishEndpointProto::new();
        proto.set_id(id);
        proto.set_p2p(self.p2p.into());
        proto
    }
}

impl From<WebRtcPublishEndpointProto> for WebRtcPublishEndpoint {
    fn from(mut proto: WebRtcPublishEndpointProto) -> Self {
        Self {
            id: proto.take_id(),
            p2p: proto.get_p2p().into(),
        }
    }
}

/// [Control API]'s `WebRtcPlayEndpoint` element representation.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize, Serialize)]
pub struct WebRtcPlayEndpoint {
    /// ID of `WebRtcPlayEndpoint`.
    #[serde(skip_deserializing)]
    id: String,

    /// URI in format `local://{room_id}/{member_id}/{endpoint_id}` pointing to
    /// [`WebRtcPublishEndpoint`] which this [`WebRtcPlayEndpoint`] plays.
    src: String,
}

impl WebRtcPlayEndpoint {
    /// Converts [`WebRtcPlayEndpoint`] into protobuf
    /// [`WebRtcPlayEndpointProto`].
    pub fn into_proto(self, id: String) -> WebRtcPlayEndpointProto {
        let mut proto = WebRtcPlayEndpointProto::new();
        proto.set_id(id);
        proto.set_src(self.src);
        proto
    }
}

impl From<WebRtcPlayEndpointProto> for WebRtcPlayEndpoint {
    fn from(mut proto: WebRtcPlayEndpointProto) -> Self {
        Self {
            id: proto.take_id(),
            src: proto.take_src(),
        }
    }
}

/// `Endpoint` element representation.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum Endpoint {
    WebRtcPublishEndpoint(WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(WebRtcPlayEndpoint),
}

impl Endpoint {
    /// Converts [`Endpoint`] into protobuf [`MemberElementProto`].
    pub fn into_proto(self, id: String) -> MemberElementProto {
        let mut proto = MemberElementProto::new();
        match self {
            Self::WebRtcPlayEndpoint(spec) => {
                proto.set_webrtc_play(spec.into_proto(id))
            }
            Self::WebRtcPublishEndpoint(spec) => {
                proto.set_webrtc_pub(spec.into_proto(id))
            }
        }
        proto
    }
}

impl From<MemberElementProto> for Endpoint {
    fn from(proto: MemberElementProto) -> Self {
        match proto.el.unwrap() {
            MemberElementOneOfEl::webrtc_pub(webrtc_pub) => {
                Self::WebRtcPublishEndpoint(webrtc_pub.into())
            }
            MemberElementOneOfEl::webrtc_play(webrtc_play) => {
                Self::WebRtcPlayEndpoint(webrtc_play.into())
            }
        }
    }
}
