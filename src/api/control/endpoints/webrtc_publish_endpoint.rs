//! `WebRtcPublishEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use derive_more::{Display, From, Into};
use serde::Deserialize;

use medea_control_api_proto::grpc::api as proto;

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

impl From<proto::web_rtc_publish_endpoint::P2p> for P2pMode {
    fn from(value: proto::web_rtc_publish_endpoint::P2p) -> Self {
        use proto::web_rtc_publish_endpoint::P2p;

        match value {
            P2p::Always => Self::Always,
            P2p::IfPossible => Self::IfPossible,
            P2p::Never => Self::Never,
        }
    }
}

impl Into<proto::web_rtc_publish_endpoint::P2p> for P2pMode {
    fn into(self) -> proto::web_rtc_publish_endpoint::P2p {
        use proto::web_rtc_publish_endpoint::P2p;

        match self {
            Self::Always => P2p::Always,
            Self::IfPossible => P2p::IfPossible,
            Self::Never => P2p::Never,
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

impl From<&proto::WebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn from(value: &proto::WebRtcPublishEndpoint) -> Self {
        Self {
            p2p: P2pMode::from(
                proto::web_rtc_publish_endpoint::P2p::from_i32(value.p2p)
                    .unwrap_or_default(),
            ),
            force_relay: value.force_relay,
        }
    }
}
