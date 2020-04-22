//! [Medea] endpoints implementations.
//!
//! [Medea]: https://github.com/instrumentisto/medea

pub mod webrtc;

use std::hash::Hash;

use derive_more::From;
use medea_client_api_proto::PeerId;
use medea_control_api_proto::grpc::api as proto;
use medea_macro::enum_delegate;

use crate::api::control::{
    callback::{url::CallbackUrl, EndpointDirection, EndpointKind},
    refs::{Fid, ToEndpoint},
};

use self::webrtc::{
    play_endpoint::WeakWebRtcPlayEndpoint,
    publish_endpoint::WeakWebRtcPublishEndpoint, WebRtcPlayEndpoint,
    WebRtcPublishEndpoint,
};

/// Enum which can store all kinds of [Medea] endpoints.
///
/// [Medea]: https://github.com/instrumentisto/medea
#[enum_delegate(pub fn any_traffic_callback_is_some(&self) -> bool)]
#[enum_delegate(pub fn is_force_relayed(&self) -> bool)]
#[derive(Clone, Debug, From)]
pub enum Endpoint {
    WebRtcPublishEndpoint(WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(WebRtcPlayEndpoint),
}

impl Endpoint {
    /// Returns [`CallbackUrl`] and [`Fid`] for the `on_stop` Control API
    /// callback of this [`Endpoint`].
    ///
    /// Also this function will change peer status of [`WebRtcPublishEndpoint`]
    /// if provided [`PeerId`] related to this kind of endpoint.
    pub fn get_on_stop(
        &self,
        peer_id: PeerId,
        kind: EndpointKind,
    ) -> Option<(Fid<ToEndpoint>, CallbackUrl)> {
        match self {
            Endpoint::WebRtcPublishEndpoint(publish) => {
                publish.get_on_stop(peer_id, kind)
            }
            Endpoint::WebRtcPlayEndpoint(play) => play.get_on_stop(kind),
        }
    }

    #[inline]
    pub fn get_direction(&self) -> EndpointDirection {
        match self {
            Endpoint::WebRtcPlayEndpoint(_) => {
                WebRtcPlayEndpoint::get_direction()
            }
            Endpoint::WebRtcPublishEndpoint(_) => {
                WebRtcPublishEndpoint::get_direction()
            }
        }
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
