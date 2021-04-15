//! [Medea] endpoints implementations.
//!
//! [Medea]: https://github.com/instrumentisto/medea

pub mod webrtc;

use derive_more::From;
use medea_control_api_proto::grpc::api as proto;
use medea_macro::enum_delegate;

use crate::signalling::elements::endpoints::webrtc::{
    play_endpoint::WeakWebRtcPlayEndpoint,
    publish_endpoint::WeakWebRtcPublishEndpoint,
};

/// Enum which can store all kinds of [Medea] endpoints.
///
/// [Medea]: https://github.com/instrumentisto/medea
#[enum_delegate(pub fn is_force_relayed(&self) -> bool)]
#[enum_delegate(pub fn has_traffic_callback(&self) -> bool)]
#[derive(Clone, Debug, From)]
pub enum Endpoint {
    WebRtcPublishEndpoint(webrtc::WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(webrtc::WebRtcPlayEndpoint),
}

impl Endpoint {
    /// Downgrades this [`Endpoint`] to [`WeakEndpoint`].
    #[inline]
    #[must_use]
    pub fn downgrade(&self) -> WeakEndpoint {
        match self {
            Self::WebRtcPublishEndpoint(publish) => publish.downgrade().into(),
            Self::WebRtcPlayEndpoint(play) => play.downgrade().into(),
        }
    }
}

impl From<Endpoint> for proto::Element {
    #[inline]
    fn from(endpoint: Endpoint) -> Self {
        match endpoint {
            Endpoint::WebRtcPublishEndpoint(play) => play.into(),
            Endpoint::WebRtcPlayEndpoint(publish) => publish.into(),
        }
    }
}

/// Weak pointer to an [`Endpoint`].
///
/// Can be upgraded to an [`Endpoint`] by calling [`WeakEndpoint::upgrade`].
#[derive(Clone, Debug, From)]
pub enum WeakEndpoint {
    /// Concrete type of this [`WeakEndpoint`] is
    /// [`WeakWebRtcPublishEndpoint`].
    WebRtcPublishEndpoint(WeakWebRtcPublishEndpoint),

    /// Concrete type of this [`WeakEndpoint`] is [`WeakWebRtcPlayEndpoint`].
    WebRtcPlayEndpoint(WeakWebRtcPlayEndpoint),
}

impl WeakEndpoint {
    /// Upgrades this weak pointer to a strong [`Endpoint`] pointer.
    #[must_use]
    pub fn upgrade(&self) -> Option<Endpoint> {
        match self {
            WeakEndpoint::WebRtcPublishEndpoint(ep) => {
                ep.safe_upgrade().map(Into::into)
            }
            WeakEndpoint::WebRtcPlayEndpoint(ep) => {
                ep.safe_upgrade().map(Into::into)
            }
        }
    }
}
