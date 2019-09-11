//! `WebRtcPublishEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: http://tiny.cc/380uaz

use derive_more::{Display, From};
use serde::Deserialize;

use medea_grpc_proto::control::{
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
    WebRtcPublishEndpoint_P2P as WebRtcPublishEndpointP2pProto,
};

/// ID of [`WebRtcPublishEndpoint`].
#[derive(Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From)]
pub struct WebRtcPublishId(pub String);

/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
#[derive(Clone, Deserialize, Debug)]
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
            WebRtcPublishEndpointP2pProto::ALWAYS => P2pMode::Always,
            WebRtcPublishEndpointP2pProto::IF_POSSIBLE => P2pMode::IfPossible,
            WebRtcPublishEndpointP2pProto::NEVER => P2pMode::Never,
        }
    }
}

impl Into<WebRtcPublishEndpointP2pProto> for P2pMode {
    fn into(self) -> WebRtcPublishEndpointP2pProto {
        match self {
            P2pMode::Always => WebRtcPublishEndpointP2pProto::ALWAYS,
            P2pMode::IfPossible => WebRtcPublishEndpointP2pProto::IF_POSSIBLE,
            P2pMode::Never => WebRtcPublishEndpointP2pProto::NEVER,
        }
    }
}

/// Media element which is able to publish media data for another client via
/// WebRTC.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode of this [`WebRtcPublishEndpoint`].
    pub p2p: P2pMode,
}

impl From<&WebRtcPublishEndpointProto> for WebRtcPublishEndpoint {
    fn from(value: &WebRtcPublishEndpointProto) -> Self {
        Self {
            p2p: P2pMode::from(value.get_p2p()),
        }
    }
}
