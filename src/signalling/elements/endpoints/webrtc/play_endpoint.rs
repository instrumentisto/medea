//! [`WebRtcPlayEndpoint`] implementation.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use chrono::{DateTime, Utc};
use medea_client_api_proto::PeerId;
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::control::{
        callback::{
            url::CallbackUrl, CallbackRequest, MediaDirection, MediaType,
            OnStartEvent, OnStopEvent, OnStopReason,
        },
        endpoints::webrtc_play_endpoint::WebRtcPlayId as Id,
        refs::SrcUri,
    },
    signalling::elements::{
        endpoints::webrtc::{
            publish_endpoint::WeakWebRtcPublishEndpoint, MediaTrafficState,
        },
        member::WeakMember,
        Member,
    },
};

use super::publish_endpoint::WebRtcPublishEndpoint;

#[derive(Debug, Clone)]
struct WebRtcPlayEndpointInner {
    /// ID of this [`WebRtcPlayEndpoint`].
    id: Id,

    /// Source URI of [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src_uri: SrcUri,

    /// Publisher [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src: WeakWebRtcPublishEndpoint,

    /// Owner [`Member`] of this [`WebRtcPlayEndpoint`].
    owner: WeakMember,

    /// [`PeerId`] of [`Peer`] created for this [`WebRtcPlayEndpoint`].
    ///
    /// Currently this field used for detecting status of this
    /// [`WebRtcPlayEndpoint`] connection.
    ///
    /// In future this may be used for removing [`WebRtcPlayEndpoint`]
    /// and related peer.
    peer_id: Option<PeerId>,

    /// Indicator whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPlayEndpoint`].
    is_force_relayed: bool,

    /// URL to which `OnStart` Control API callback will be sent.
    on_start: Option<CallbackUrl>,

    /// URL to which `OnStop` Control API callback will be sent.
    on_stop: Option<CallbackUrl>,

    /// Current [`MediaTrafficState`] of this [`WebRtcPlayEndpoint`].
    media_traffic_state: MediaTrafficState,

    /// [`MediaTrafficState`] which should be set as current after
    /// [`WebRtcPlayEndpoint`] will be started by calling
    /// [`WebRtcPlayEndpoint::get_on_start`].
    on_start_media_traffic_state: Option<MediaTrafficState>,
}

impl WebRtcPlayEndpointInner {
    fn src_uri(&self) -> SrcUri {
        self.src_uri.clone()
    }

    fn owner(&self) -> Member {
        self.owner.upgrade()
    }

    fn weak_owner(&self) -> WeakMember {
        self.owner.clone()
    }

    fn src(&self) -> WebRtcPublishEndpoint {
        self.src.upgrade()
    }

    fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = Some(peer_id)
    }

    fn peer_id(&self) -> Option<PeerId> {
        self.peer_id
    }

    fn reset(&mut self) {
        self.peer_id = None
    }
}

impl Drop for WebRtcPlayEndpointInner {
    fn drop(&mut self) {
        if let Some(receiver_publisher) = self.src.safe_upgrade() {
            receiver_publisher.remove_empty_weaks_from_sinks();
        }
    }
}

/// Signalling representation of Control API's [`WebRtcPlayEndpoint`].
///
/// [`WebRtcPlayEndpoint`]: crate::api::control::endpoints::WebRtcPlayEndpoint
#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpoint(Rc<RefCell<WebRtcPlayEndpointInner>>);

impl WebRtcPlayEndpoint {
    /// [`MediaDirection`] of the [`WebRtcPlayEndpoint`].
    pub const DIRECTION: MediaDirection = MediaDirection::Play;

    /// Creates new [`WebRtcPlayEndpoint`].
    pub fn new(
        id: Id,
        src_uri: SrcUri,
        publisher: WeakWebRtcPublishEndpoint,
        owner: WeakMember,
        is_force_relayed: bool,
        on_start: Option<CallbackUrl>,
        on_stop: Option<CallbackUrl>,
    ) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPlayEndpointInner {
            id,
            src_uri,
            src: publisher,
            owner,
            peer_id: None,
            is_force_relayed,
            on_start,
            on_stop,
            media_traffic_state: MediaTrafficState::new(),
            on_start_media_traffic_state: Some(
                MediaTrafficState::with_media_type(MediaType::Both),
            ),
        })))
    }

    /// Returns [`SrcUri`] of this [`WebRtcPlayEndpoint`].
    pub fn src_uri(&self) -> SrcUri {
        self.0.borrow().src_uri()
    }

    /// Returns owner [`Member`] of this [`WebRtcPlayEndpoint`].
    ///
    /// __This function will panic if pointer to [`Member`] was dropped.__
    pub fn owner(&self) -> Member {
        self.0.borrow().owner()
    }

    /// Returns weak pointer to owner [`Member`] of this
    /// [`WebRtcPlayEndpoint`].
    pub fn weak_owner(&self) -> WeakMember {
        self.0.borrow().weak_owner()
    }

    /// Returns srcs's [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if weak pointer was dropped.__
    pub fn src(&self) -> WebRtcPublishEndpoint {
        self.0.borrow().src()
    }

    /// Saves [`PeerId`] of this [`WebRtcPlayEndpoint`].
    pub fn set_peer_id(&self, peer_id: PeerId) {
        self.0.borrow_mut().set_peer_id(peer_id);
    }

    /// Returns [`PeerId`] of this [`WebRtcPlayEndpoint`]'s [`Peer`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    pub fn peer_id(&self) -> Option<PeerId> {
        self.0.borrow().peer_id()
    }

    /// Resets state of this [`WebRtcPlayEndpoint`].
    ///
    /// _Atm this only resets [`PeerId`]._
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Returns [`Id`] of this [`WebRtcPlayEndpoint`].
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Indicates whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPlayEndpoint`].
    pub fn is_force_relayed(&self) -> bool {
        self.0.borrow().is_force_relayed
    }

    /// Returns [`CallbackUrl`] to which Medea should send `OnStart` callback.
    ///
    /// Returns `None` if [`WebRtcPlayEndpoint::waiting_for_start_mute_state`]
    /// is `None`.
    ///
    /// Sets [`WebRtcPlayEndpoint::mute_state`] to
    /// [`WebRtcPlayEndpoint::waiting_for_start_mute_state`] if it `Some`.
    pub fn get_on_start(
        &self,
        at: DateTime<Utc>,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        let mut inner = self.0.borrow_mut();
        if let Some(awaits_on_start) = inner.on_start_media_traffic_state {
            if inner.media_traffic_state == awaits_on_start {
                return None;
            }
            inner.media_traffic_state = awaits_on_start;
            inner.on_start_media_traffic_state = None;
            let fid =
                inner.owner().get_fid_to_endpoint(inner.id.clone().into());

            if let Some(on_start) = inner.on_start.clone() {
                return Some((
                    on_start,
                    CallbackRequest::new(
                        fid,
                        OnStartEvent {
                            direction: Self::DIRECTION,
                            media_type: awaits_on_start.into_media_type()?,
                        },
                        at,
                    ),
                ));
            }
        }

        None
    }

    /// Returns `true` if `on_start` or `on_stop` callback is set.
    pub fn has_traffic_callback(&self) -> bool {
        let inner = self.0.borrow();
        inner.on_stop.is_some() || inner.on_start.is_some()
    }

    /// Returns [`CallbackUrl`] and [`Fid`] for the `on_stop` Control API
    /// callback of this [`WebRtcPlayEndpoint`].
    ///
    /// Sets provided [`MediaType`] to stopped in the
    /// [`WebRtcPlayEndpoint::media_traffic_state`].
    ///
    /// If provided [`MediaType`] will be already stopped then `None` will be
    /// returned.
    pub fn get_on_stop(
        &self,
        at: DateTime<Utc>,
        media_type: MediaType,
        reason: OnStopReason,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        let mut inner = self.0.borrow_mut();
        if !inner.media_traffic_state.is_stopped(media_type) {
            inner.media_traffic_state.stopped(media_type);

            let fid =
                inner.owner().get_fid_to_endpoint(inner.id.clone().into());
            if let Some(url) = inner.on_stop.clone() {
                return Some((
                    url,
                    CallbackRequest::new(
                        fid,
                        OnStopEvent {
                            reason,
                            media_type,
                            media_direction: Self::DIRECTION,
                        },
                        at,
                    ),
                ));
            }
        }

        None
    }

    /// Sets [`WebRtcPlayEndpoint::on_start_media_traffic_state`] to the
    /// [`MediaTrafficState`] in which provided [`MediaType`] will be
    /// started.
    ///
    /// When [`WebRtcPlayEndpoint::get_on_start`] will be called then
    /// [`WebRtcPlayEndpoint::media_traffic_state`] will be set to the
    /// [`WebRtcPlayEndpoint::on_start_media_traffic_state`].
    pub fn set_on_start_media_traffic_state(&self, media_type: MediaType) {
        let mut inner = self.0.borrow_mut();
        if let Some(s) = inner.on_start_media_traffic_state.as_mut() {
            s.started(media_type);
        } else {
            inner.on_start_media_traffic_state =
                Some(MediaTrafficState::with_media_type(media_type));
        }
    }

    /// Downgrades [`WebRtcPlayEndpoint`] to [`WeakWebRtcPlayEndpoint`] weak
    /// pointer.
    pub fn downgrade(&self) -> WeakWebRtcPlayEndpoint {
        WeakWebRtcPlayEndpoint(Rc::downgrade(&self.0))
    }

    /// Compares [`WebRtcPlayEndpoint`]'s inner pointers. If both pointers
    /// points to the same address, then returns `true`.
    #[cfg(test)]
    pub fn ptr_eq(&self, another_play: &Self) -> bool {
        Rc::ptr_eq(&self.0, &another_play.0)
    }
}

/// Weak pointer to [`WebRtcPlayEndpoint`].
#[derive(Debug, Clone)]
pub struct WeakWebRtcPlayEndpoint(Weak<RefCell<WebRtcPlayEndpointInner>>);

impl WeakWebRtcPlayEndpoint {
    /// Upgrades weak pointer to strong pointer.
    ///
    /// # Panics
    ///
    /// If weak pointer has been dropped.
    pub fn upgrade(&self) -> WebRtcPlayEndpoint {
        WebRtcPlayEndpoint(self.0.upgrade().unwrap())
    }

    /// Upgrades to [`WebRtcPlayEndpoint`] safely.
    ///
    /// Returns `None` if weak pointer has been dropped.
    pub fn safe_upgrade(&self) -> Option<WebRtcPlayEndpoint> {
        self.0.upgrade().map(WebRtcPlayEndpoint)
    }
}

impl Into<proto::member::Element> for WebRtcPlayEndpoint {
    fn into(self) -> proto::member::Element {
        proto::member::Element {
            el: Some(proto::member::element::El::WebrtcPlay(self.into())),
        }
    }
}

impl Into<proto::WebRtcPlayEndpoint> for WebRtcPlayEndpoint {
    fn into(self) -> proto::WebRtcPlayEndpoint {
        proto::WebRtcPlayEndpoint {
            on_start: self
                .0
                .borrow()
                .on_start
                .as_ref()
                .map(ToString::to_string),
            on_stop: self.0.borrow().on_stop.as_ref().map(ToString::to_string),
            src: self.src_uri().to_string(),
            id: self.id().to_string(),
            force_relay: self.is_force_relayed(),
        }
    }
}

impl Into<proto::Element> for WebRtcPlayEndpoint {
    fn into(self) -> proto::Element {
        proto::Element {
            el: Some(proto::element::El::WebrtcPlay(self.into())),
        }
    }
}
