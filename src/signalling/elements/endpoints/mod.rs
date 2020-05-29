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
#[derive(Clone, Debug, From)]
pub enum Endpoint {
    WebRtcPublishEndpoint(webrtc::WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(webrtc::WebRtcPlayEndpoint),
}

impl Endpoint {
    /// Returns `true` if `on_start` or `on_stop` callback is set.
    #[allow(clippy::unused_self)]
    #[inline]
    pub fn has_traffic_callback(&self) -> bool {
        // TODO: Delegate this call to
        //       `WebRtcPublishEndpoint`/`WebRtcPlayEndpoint`.

        false
    }

    /// Returns [`Weak`] reference to this [`Endpoint`].
    pub fn downgrade(&self) -> WeakEndpoint {
        match self {
            Self::WebRtcPublishEndpoint(publish) => publish.downgrade().into(),
            Self::WebRtcPlayEndpoint(play) => play.downgrade().into(),
        }
    }
}

impl Into<proto::Element> for Endpoint {
    fn into(self) -> proto::Element {
        match self {
            Self::WebRtcPublishEndpoint(play) => play.into(),
            Self::WebRtcPlayEndpoint(publish) => publish.into(),
        }
    }
}

/// Weak pointer to an [`Endpoint`].
///
/// Can be upgraded to an [`Endpoint`] by calling [`WeakEndpoint::upgrade`].
#[derive(Clone, Debug, From)]
pub enum WeakEndpoint {
    /// [`Weak`] pointer to the [`WebRtcPublishEndpoint`].
    WebRtcPublishEndpoint(WeakWebRtcPublishEndpoint),

    /// [`Weak`] pointer to the [`WebRtcPlayEndpoint`].
    WebRtcPlayEndpoint(WeakWebRtcPlayEndpoint),
}

impl WeakEndpoint {
    /// Upgrades this weak pointer to a strong [`Endpoint`] pointer.
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
