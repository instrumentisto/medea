//! [Medea] endpoints implementations.
//!
//! [Medea]: https://github.com/instrumentisto/medea

pub mod webrtc;

use chrono::{DateTime, Utc};
use derive_more::From;
use medea_client_api_proto::PeerId;
use medea_control_api_proto::grpc::api as proto;
use medea_macro::enum_delegate;

use crate::api::control::callback::{
    url::CallbackUrl, CallbackRequest, EndpointDirection, EndpointKind,
    OnStopReason,
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
#[enum_delegate(pub fn awaits_starting(&self, kind: EndpointKind))]
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
        at: DateTime<Utc>,
        kind: EndpointKind,
        reason: OnStopReason,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        match self {
            Endpoint::WebRtcPublishEndpoint(publish) => {
                publish.get_on_stop(peer_id, at, kind, reason)
            }
            Endpoint::WebRtcPlayEndpoint(play) => {
                play.get_on_stop(at, kind, reason)
            }
        }
    }

    #[inline]
    pub fn get_direction(&self) -> EndpointDirection {
        match self {
            Endpoint::WebRtcPlayEndpoint(_) => WebRtcPlayEndpoint::DIRECTION,
            Endpoint::WebRtcPublishEndpoint(_) => {
                WebRtcPublishEndpoint::DIRECTION
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
    pub fn get_traffic_flowing_on_stop(
        &self,
        peer_id: PeerId,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        self.upgrade()
            .map(|e| {
                e.awaits_starting(EndpointKind::Both);
                e.get_on_stop(
                    peer_id,
                    Utc::now(),
                    EndpointKind::Both,
                    OnStopReason::TrafficNotFlowing,
                )
            })
            .flatten()
    }

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
