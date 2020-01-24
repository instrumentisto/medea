//! `WebRtcPublishEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use derive_more::{Display, From, Into};
use serde::Deserialize;

use medea_control_api_proto::grpc::medea::{
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
    web_rtc_publish_endpoint::P2p as WebRtcPublishEndpointP2pProto,
};

/// ID of [`WebRtcPublishEndpoint`].
#[derive(
    Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From, Into,
)]
pub struct WebRtcPublishId(String);

/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Deserialize, Debug)]
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,

    /// Never connect peer-to-peer.
    Never,

    /// Connect peer-to-peer if it possible.
    IfPossible,
}

impl From<WebRtcPublishEndpointP2pProto> for P2pMode {
    fn from(value: WebRtcPublishEndpointP2pProto) -> Self {
        match value {
            WebRtcPublishEndpointP2pProto::ALWAYS => Self::Always,
            WebRtcPublishEndpointP2pProto::IF_POSSIBLE => Self::IfPossible,
            WebRtcPublishEndpointP2pProto::NEVER => Self::Never,
        }
    }
}

impl Into<WebRtcPublishEndpointP2pProto> for P2pMode {
    fn into(self) -> WebRtcPublishEndpointP2pProto {
        match self {
            Self::Always => WebRtcPublishEndpointP2pProto::ALWAYS,
            Self::IfPossible => WebRtcPublishEndpointP2pProto::IF_POSSIBLE,
            Self::Never => WebRtcPublishEndpointP2pProto::NEVER,
        }
    }
}

/// Media element which is able to publish media data for another client via
/// WebRTC.
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode of this [`WebRtcPublishEndpoint`].
    pub p2p: P2pMode,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    pub force_relay: bool,
}

impl From<&WebRtcPublishEndpointProto> for WebRtcPublishEndpoint {
    fn from(value: &WebRtcPublishEndpointProto) -> Self {
        Self {
            p2p: P2pMode::from(value.get_p2p()),
            force_relay: value.get_force_relay(),
        }
    }
}
