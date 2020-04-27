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
    url::CallbackUrl, CallbackRequest, MediaType, OnStopReason,
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
#[enum_delegate(
    pub fn set_on_start_media_traffic_state(&self, media_type: MediaType)
)]
#[derive(Clone, Debug, From)]
pub enum Endpoint {
    WebRtcPublishEndpoint(WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(WebRtcPlayEndpoint),
}

impl Endpoint {
    /// Returns [`CallbackUrl`] and [`CallbackRequest`] for the `on_stop`
    /// Control API callback of this [`Endpoint`].
    ///
    /// Also this function will change peer status of [`WebRtcPublishEndpoint`]
    /// if provided [`PeerId`] related to this kind of endpoint.
    ///
    /// `None` will be returned if `on_stop` shouldn't be sent.
    pub fn get_on_stop(
        &self,
        peer_id: PeerId,
        at: DateTime<Utc>,
        media_type: MediaType,
        reason: OnStopReason,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        match self {
            Endpoint::WebRtcPublishEndpoint(publish) => {
                publish.get_on_stop(peer_id, at, media_type, reason)
            }
            Endpoint::WebRtcPlayEndpoint(play) => {
                play.get_on_stop(at, media_type, reason)
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
    /// Returns [`CallbackUrl`] and [`CallbackRequest`] for this
    /// [`WeakEndpoint`] with [`OnStopReason::TrafficNotFlowing`]
    /// and provided [`DateTime`].
    ///
    /// `None` will be returned if `on_stop` Control API callback for this
    /// [`WeakEndpoint`] shouldn't be sent.
    pub fn get_traffic_not_flowing_on_stop(
        &self,
        peer_id: PeerId,
        at: DateTime<Utc>,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        self.get_both_on_stop(peer_id, OnStopReason::TrafficNotFlowing, at)
    }

    /// Returns [`CallbackUrl`] and [`CallbackRequest`] for this
    /// [`WeakEndpoint`] with provided [`OnStopReason`], [`DateTime`].
    ///
    /// `None` will be returned if `on_stop` Control API callback for this
    /// [`WeakEndpoint`] shouldn't be sent.
    pub fn get_both_on_stop(
        &self,
        peer_id: PeerId,
        reason: OnStopReason,
        at: DateTime<Utc>,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        self.upgrade()
            .map(|e| {
                e.set_on_start_media_traffic_state(MediaType::Both);
                e.get_on_stop(peer_id, at, MediaType::Both, reason)
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
