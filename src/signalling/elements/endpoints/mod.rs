//! [Medea] endpoints implementations.
//!
//! [Medea]: https://github.com/instrumentisto/medea

pub mod webrtc;

use derive_more::From;
use medea_control_api_proto::grpc::api as proto;

use crate::signalling::elements::endpoints::webrtc::{
    play_endpoint::WeakWebRtcPlayEndpoint,
    publish_endpoint::WeakWebRtcPublishEndpoint,
};

/// Enum which can store all kinds of [Medea] endpoints.
///
/// [Medea]: https://github.com/instrumentisto/medea
#[derive(Clone, Debug, From)]
pub enum Endpoint {
    WebRtcPublishEndpoint(webrtc::WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(webrtc::WebRtcPlayEndpoint),
}

impl Endpoint {
    /// Indicates whether only `relay` ICE candidates are allowed for this
    /// [`Endpoint`].
    pub fn is_force_relayed(&self) -> bool {
        match self {
            Endpoint::WebRtcPublishEndpoint(publish) => {
                publish.is_force_relayed()
            }
            Endpoint::WebRtcPlayEndpoint(play) => play.is_force_relayed(),
        }
    }

    /// Returns `true` if `on_start` or `on_stop` callback is set.
    #[allow(clippy::unused_self)]
    pub fn any_traffic_callback_is_some(&self) -> bool {
        // TODO: delegate this call to the
        // WebRtcPublishEndpoint/WebRtcPlayEndpoint.

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

/// Weak pointer to a [`Endpoint`].
///
/// Can be upgraded to the [`Endpoint`] by calling [`WeakEndpoint::upgrade`].
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
            WeakEndpoint::WebRtcPublishEndpoint(publish_endpoint) => {
                publish_endpoint.safe_upgrade().map(|e| e.into())
            }
            WeakEndpoint::WebRtcPlayEndpoint(play_endpoint) => {
                play_endpoint.safe_upgrade().map(|e| e.into())
            }
        }
    }
}
