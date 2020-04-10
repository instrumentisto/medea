//! [Medea] endpoints implementations.
//!
//! [Medea]: https://github.com/instrumentisto/medea

pub mod webrtc;

use derive_more::From;
use medea_control_api_proto::grpc::api as proto;
use medea_macro::enum_delegate;

use self::webrtc::{
    play_endpoint::WeakWebRtcPlayEndpoint,
    publish_endpoint::WeakWebRtcPublishEndpoint, WebRtcPlayEndpoint,
    WebRtcPublishEndpoint,
};

/// Enum which can store all kinds of [Medea] endpoints.
///
/// [Medea]: https://github.com/instrumentisto/medea
#[enum_delegate(pub fn is_some_traffic_callbacks(&self) -> bool)]
#[enum_delegate(pub fn is_force_relayed(&self) -> bool)]
#[derive(Clone, Debug, From)]
pub enum Endpoint {
    WebRtcPublishEndpoint(WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(WebRtcPlayEndpoint),
}

impl Into<proto::Element> for Endpoint {
    fn into(self) -> proto::Element {
        match self {
            Self::WebRtcPublishEndpoint(play) => play.into(),
            Self::WebRtcPlayEndpoint(publish) => publish.into(),
        }
    }
}

/// Weak pointer to a some endpoint.
///
/// Can be upgraded to the [`Endpoint`] by calling [`WeakEndpoint::upgrade`].
#[derive(Clone, Debug, From)]
pub enum WeakEndpoint {
    /// Weak pointer to the [`WebRtcPublishEndpoint`].
    WebRtcPublishEndpoint(WeakWebRtcPublishEndpoint),

    /// Weak pointer to the [`WebRtcPlayEndpoint`].
    WebRtcPlayEndpoint(WeakWebRtcPlayEndpoint),
}

impl WeakEndpoint {
    /// Upgrades this weak pointer to a strong [`Endpoint`] pointer.
    pub fn upgrade(&self) -> Option<Endpoint> {
        match self {
            WeakEndpoint::WebRtcPublishEndpoint(publish_endpoint) => {
                publish_endpoint.safe_upgrade().map(|e| e.into())
            }
            WeakEndpoint::WebRtcPlayEndpoint(play_endpoint) => {
                play_endpoint.safe_upgrade().map(|e| e.into())
            }
        }
    }
}
