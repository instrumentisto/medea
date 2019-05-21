//! Element definitions and implementations.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
pub enum Element {
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint },
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint},
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Media element which is able to play media data for client via WebRTC.
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode.
    pub p2p: P2pMode,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebRtcPlayEndpoint {
    pub src: String,
}
