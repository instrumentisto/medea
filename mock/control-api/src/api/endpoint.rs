//! `Endpoint` related methods and entities.

use medea_control_api_proto::grpc::api as proto;
use serde::{Deserialize, Serialize};

/// P2P mode of [`WebRtcPublishEndpoint`].
#[derive(Debug, Deserialize, Serialize)]
pub enum P2pMode {
    Always,
    Never,
    IfPossible,
}

impl Into<proto::web_rtc_publish_endpoint::P2p> for P2pMode {
    fn into(self) -> proto::web_rtc_publish_endpoint::P2p {
        use proto::web_rtc_publish_endpoint::P2p::*;
        match self {
            Self::Always => Always,
            Self::IfPossible => IfPossible,
            Self::Never => Never,
        }
    }
}

impl From<proto::web_rtc_publish_endpoint::P2p> for P2pMode {
    fn from(proto: proto::web_rtc_publish_endpoint::P2p) -> Self {
        use proto::web_rtc_publish_endpoint::P2p::*;
        match proto {
            Always => Self::Always,
            IfPossible => Self::IfPossible,
            Never => Self::Never,
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

    /// URL to which `OnStart` Control API callback will be sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    on_start: Option<String>,

    /// URL to which `OnStop` Control API callback will be sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    on_stop: Option<String>,
}

impl WebRtcPublishEndpoint {
    /// Converts [`WebRtcPublishEndpoint`] into protobuf
    /// [`proto::WebRtcPublishEndpoint`].
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::WebRtcPublishEndpoint {
        let p2p: proto::web_rtc_publish_endpoint::P2p = self.p2p.into();
        proto::WebRtcPublishEndpoint {
            id,
            p2p: p2p as i32,
            force_relay: self.force_relay,
            on_start: self.on_start.unwrap_or_default(),
            on_stop: self.on_stop.unwrap_or_default(),
        }
    }
}

impl From<proto::WebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn from(proto: proto::WebRtcPublishEndpoint) -> Self {
        Self {
            id: proto.id,
            p2p: proto::web_rtc_publish_endpoint::P2p::from_i32(proto.p2p)
                .unwrap_or_default()
                .into(),
            force_relay: proto.force_relay,
            on_start: Some(proto.on_start).filter(|s| !s.is_empty()),
            on_stop: Some(proto.on_stop).filter(|s| !s.is_empty()),
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

    /// URL to which `OnStart` Control API callback will be sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    on_start: Option<String>,

    /// URL to which `OnStop` Control API callback will be sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    on_stop: Option<String>,
}

impl WebRtcPlayEndpoint {
    /// Converts [`WebRtcPlayEndpoint`] into protobuf
    /// [`proto::WebRtcPlayEndpoint`].
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::WebRtcPlayEndpoint {
        proto::WebRtcPlayEndpoint {
            id,
            src: self.src,
            force_relay: self.force_relay,
            on_start: self.on_start.unwrap_or_default(),
            on_stop: self.on_stop.unwrap_or_default(),
        }
    }
}

impl From<proto::WebRtcPlayEndpoint> for WebRtcPlayEndpoint {
    fn from(proto: proto::WebRtcPlayEndpoint) -> Self {
        Self {
            id: proto.id,
            src: proto.src,
            force_relay: proto.force_relay,
            on_start: Some(proto.on_start).filter(|s| !s.is_empty()),
            on_stop: Some(proto.on_stop).filter(|s| !s.is_empty()),
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
    /// Converts [`Endpoint`] into protobuf [`proto::member::Element`].
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::member::Element {
        let el = match self {
            Self::WebRtcPlayEndpoint(spec) => {
                proto::member::element::El::WebrtcPlay(spec.into_proto(id))
            }
            Self::WebRtcPublishEndpoint(spec) => {
                proto::member::element::El::WebrtcPub(spec.into_proto(id))
            }
        };
        proto::member::Element { el: Some(el) }
    }
}

impl From<proto::member::Element> for Endpoint {
    fn from(proto: proto::member::Element) -> Self {
        match proto.el.unwrap() {
            proto::member::element::El::WebrtcPub(webrtc_pub) => {
                Self::WebRtcPublishEndpoint(webrtc_pub.into())
            }
            proto::member::element::El::WebrtcPlay(webrtc_play) => {
                Self::WebRtcPlayEndpoint(webrtc_play.into())
            }
        }
    }
}
