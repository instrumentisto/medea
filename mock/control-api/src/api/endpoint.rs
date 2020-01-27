//! `Endpoint` related methods and entities.

use medea_control_api_proto::grpc::medea::{
    member::{
        element::El as MemberElementOneOfEl, Element as MemberElementProto,
    },
    web_rtc_publish_endpoint::P2p as P2pModeProto,
    WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
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
            Self::Always => P2pModeProto::Always,
            Self::IfPossible => P2pModeProto::IfPossible,
            Self::Never => P2pModeProto::Never,
        }
    }
}

impl From<P2pModeProto> for P2pMode {
    fn from(proto: P2pModeProto) -> Self {
        match proto {
            P2pModeProto::Always => Self::Always,
            P2pModeProto::IfPossible => Self::IfPossible,
            P2pModeProto::Never => Self::Never,
        }
    }
}

/// [Control API]'s `WebRtcPublishEndpoint` representation.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Deserialize, Serialize)]
pub struct WebRtcPublishEndpoint {
    /// ID of [`WebRtcPublishEndpoint`].
    #[serde(skip_deserializing)]
    id: String,

    /// Mode of connection for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    force_relay: bool,
}

impl WebRtcPublishEndpoint {
    /// Converts [`WebRtcPublishEndpoint`] into protobuf
    /// [`WebRtcPublishEndpointProto`].
    #[must_use]
    pub fn into_proto(self, id: String) -> WebRtcPublishEndpointProto {
        let p2p: P2pModeProto = self.p2p.into();
        WebRtcPublishEndpointProto {
            id,
            p2p: p2p as i32,
            force_relay: self.force_relay,
            on_start: String::new(),
            on_stop: String::new(),
        }
    }
}

impl From<WebRtcPublishEndpointProto> for WebRtcPublishEndpoint {
    fn from(proto: WebRtcPublishEndpointProto) -> Self {
        Self {
            id: proto.id,
            p2p: P2pModeProto::from_i32(proto.p2p).unwrap_or_default().into(),
            force_relay: proto.force_relay,
        }
    }
}

/// [Control API]'s `WebRtcPlayEndpoint` element representation.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Deserialize, Serialize)]
pub struct WebRtcPlayEndpoint {
    /// ID of `WebRtcPlayEndpoint`.
    #[serde(skip_deserializing)]
    id: String,

    /// URI in format `local://{room_id}/{member_id}/{endpoint_id}` pointing to
    /// [`WebRtcPublishEndpoint`] which this [`WebRtcPlayEndpoint`] plays.
    src: String,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    force_relay: bool,
}

impl WebRtcPlayEndpoint {
    /// Converts [`WebRtcPlayEndpoint`] into protobuf
    /// [`WebRtcPlayEndpointProto`].
    #[must_use]
    pub fn into_proto(self, id: String) -> WebRtcPlayEndpointProto {
        WebRtcPlayEndpointProto {
            id,
            src: self.src,
            force_relay: self.force_relay,
            on_start: String::new(),
            on_stop: String::new(),
        }
    }
}

impl From<WebRtcPlayEndpointProto> for WebRtcPlayEndpoint {
    fn from(proto: WebRtcPlayEndpointProto) -> Self {
        Self {
            id: proto.id,
            src: proto.src,
            force_relay: proto.force_relay,
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
    #[must_use]
    pub fn into_proto(self, id: String) -> MemberElementProto {
        let oneof = match self {
            Self::WebRtcPlayEndpoint(spec) => {
                MemberElementOneOfEl::WebrtcPlay(spec.into_proto(id))
            }
            Self::WebRtcPublishEndpoint(spec) => {
                MemberElementOneOfEl::WebrtcPub(spec.into_proto(id))
            }
        };

        MemberElementProto { el: Some(oneof) }
    }
}

impl From<MemberElementProto> for Endpoint {
    fn from(proto: MemberElementProto) -> Self {
        match proto.el.unwrap() {
            MemberElementOneOfEl::WebrtcPub(webrtc_pub) => {
                Self::WebRtcPublishEndpoint(webrtc_pub.into())
            }
            MemberElementOneOfEl::WebrtcPlay(webrtc_play) => {
                Self::WebRtcPlayEndpoint(webrtc_play.into())
            }
        }
    }
}
