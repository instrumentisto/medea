//! [`WebRtcPublishEndpoint`] implementation.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
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
        endpoints::webrtc_publish_endpoint::{P2pMode, WebRtcPublishId as Id},
    },
    signalling::{
        elements::{
            endpoints::webrtc::play_endpoint::WeakWebRtcPlayEndpoint,
            member::WeakMember, Member,
        },
        peers::media_traffic_state::{
            get_diff_disabled, get_diff_enabled, MediaTrafficState,
        },
    },
};

use super::play_endpoint::WebRtcPlayEndpoint;

#[derive(Clone, Debug)]
struct WebRtcPublishEndpointInner {
    /// ID of this [`WebRtcPublishEndpoint`].
    id: Id,

    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// Indicator whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPublishEndpoint`].
    is_force_relayed: bool,

    /// All sinks of this [`WebRtcPublishEndpoint`].
    sinks: Vec<WeakWebRtcPlayEndpoint>,

    /// Owner [`Member`] of this [`WebRtcPublishEndpoint`].
    owner: WeakMember,

    /// Publishing statuses of the [`Peer`]s related to this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// `true` value indicated that [`Peer`] is publishes some media traffic.
    ///
    /// Also this field acts as store of all [`PeerId`]s related to this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// [`Peer`]: crate::media::peer:Peer
    peers_publishing_statuses: HashMap<PeerId, bool>,

    /// URL to which `OnStart` Control API callback will be sent.
    on_start: Option<CallbackUrl>,

    /// URL to which `OnStop` Control API callback will be sent.
    on_stop: Option<CallbackUrl>,

    /// Current [`MediaTrafficState`] of this [`WebRtcPublishEndpoint`].
    media_traffic_state: MediaTrafficState,

    /// Mute state of this [`WebRtcPublishEndpoint`].
    mute_state: MediaTrafficState,
}

impl Drop for WebRtcPublishEndpointInner {
    fn drop(&mut self) {
        for receiver in self
            .sinks
            .iter()
            .filter_map(WeakWebRtcPlayEndpoint::safe_upgrade)
        {
            if let Some(receiver_owner) = receiver.weak_owner().safe_upgrade() {
                receiver_owner.remove_sink(&receiver.id())
            }
        }
    }
}

impl WebRtcPublishEndpointInner {
    fn add_sinks(&mut self, sink: WeakWebRtcPlayEndpoint) {
        self.sinks.push(sink);
    }

    fn sinks(&self) -> Vec<WebRtcPlayEndpoint> {
        self.sinks
            .iter()
            .map(WeakWebRtcPlayEndpoint::upgrade)
            .collect()
    }

    fn owner(&self) -> Member {
        self.owner.upgrade()
    }

    fn peer_ids(&self) -> HashSet<PeerId> {
        self.peers_publishing_statuses.keys().cloned().collect()
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.peers_publishing_statuses.remove(peer_id);
    }

    fn remove_peer_ids(&mut self, peer_ids: &[PeerId]) {
        for peer_id in peer_ids {
            self.remove_peer_id(peer_id)
        }
    }
}

/// Signalling representation of [`WebRtcPublishEndpoint`].
///
/// [`WebRtcPublishEndpoint`]:
/// crate::api::control::endpoints::WebRtcPublishEndpoint
#[derive(Debug, Clone)]
pub struct WebRtcPublishEndpoint(Rc<RefCell<WebRtcPublishEndpointInner>>);

impl WebRtcPublishEndpoint {
    /// [`MediaDirection`] of the [`WebRtcPublishEndpoint`].
    pub const DIRECTION: MediaDirection = MediaDirection::Publish;

    /// Creates new [`WebRtcPublishEndpoint`].
    pub fn new(
        id: Id,
        p2p: P2pMode,
        owner: WeakMember,
        is_force_relayed: bool,
        on_start: Option<CallbackUrl>,
        on_stop: Option<CallbackUrl>,
    ) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPublishEndpointInner {
            id,
            p2p,
            is_force_relayed,
            sinks: Vec::new(),
            owner,
            peers_publishing_statuses: HashMap::new(),
            on_start,
            on_stop,
            mute_state: MediaTrafficState::new(),
            media_traffic_state: MediaTrafficState::new(),
        })))
    }

    /// Adds [`WebRtcPlayEndpoint`] (sink) to this [`WebRtcPublishEndpoint`].
    pub fn add_sink(&self, sink: WeakWebRtcPlayEndpoint) {
        self.0.borrow_mut().add_sinks(sink)
    }

    /// Returns all [`WebRtcPlayEndpoint`]s (sinks) of this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// # Panics
    ///
    /// If meets empty pointer.
    pub fn sinks(&self) -> Vec<WebRtcPlayEndpoint> {
        self.0.borrow().sinks()
    }

    /// Returns owner [`Member`] of this [`WebRtcPublishEndpoint`].
    ///
    /// # Panics
    ///
    /// If pointer to [`Member`] has been dropped.
    pub fn owner(&self) -> Member {
        self.0.borrow().owner()
    }

    /// Adds [`PeerId`] related to this [`WebRtcPublishEndpoint`].
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0
            .borrow_mut()
            .peers_publishing_statuses
            .insert(peer_id, false);
    }

    /// Returns all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.borrow().peer_ids()
    }

    /// Removes all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    pub fn remove_peer_ids(&self, peer_ids: &[PeerId]) {
        self.0.borrow_mut().remove_peer_ids(peer_ids)
    }

    /// Returns [`Id`] of this [`WebRtcPublishEndpoint`].
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Removes all dropped [`Weak`] pointers from sinks of this
    /// [`WebRtcPublishEndpoint`].
    pub fn remove_empty_weaks_from_sinks(&self) {
        self.0
            .borrow_mut()
            .sinks
            .retain(|e| e.safe_upgrade().is_some());
    }

    /// Peer-to-peer mode of this [`WebRtcPublishEndpoint`].
    pub fn p2p(&self) -> P2pMode {
        self.0.borrow().p2p
    }

    /// Indicates whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPublishEndpoint`].
    pub fn is_force_relayed(&self) -> bool {
        self.0.borrow().is_force_relayed
    }

    /// Sets publishing status of the provided [`PeerId`] related to this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// If provided [`PeerId`] is not related to this [`WebRtcPublishEndpoint`]
    /// then nothing will be done.
    pub fn set_peer_status(&self, peer_id: PeerId, is_publishing: bool) {
        if let Some(peer_status) = self
            .0
            .borrow_mut()
            .peers_publishing_statuses
            .get_mut(&peer_id)
        {
            *peer_status = is_publishing;
        }
    }

    /// Returns `true` if at least one [`PeerConnection`] related to this
    /// [`WebRtcPublishEndpoint`] is publishing.
    pub fn is_endpoint_publishing(&self) -> bool {
        self.0
            .borrow()
            .peers_publishing_statuses
            .values()
            .any(|status| *status)
    }

    /// Returns count of [`Peer`]s which are publishes.
    pub fn publishing_peers_count(&self) -> usize {
        self.0
            .borrow()
            .peers_publishing_statuses
            .values()
            .filter(|s| **s)
            .count()
    }

    /// Returns [`CallbackUrl`] to which Medea should send `OnStart` callback.
    ///
    /// Returns `None` if
    /// [`WebRtcPublishEndpoint::waiting_for_start_mute_state`] is `None`.
    ///
    /// Sets [`WebRtcPublishEndpoint::mute_state`] to
    /// [`WebRtcPublishEndpoint::waiting_for_start_mute_state`] if it `Some`.
    pub fn get_on_start(
        &self,
        at: DateTime<Utc>,
        media_type: MediaType,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        let mut inner = self.0.borrow_mut();

        let old_media_traffic_state = inner.media_traffic_state;
        let mut new_media_traffic_state = inner.media_traffic_state;
        new_media_traffic_state.started(media_type);

        if let Some(started_media_type) =
            get_diff_enabled(old_media_traffic_state, new_media_traffic_state)
        {
            if inner.mute_state.is_disabled(started_media_type) {
                inner.media_traffic_state = new_media_traffic_state;
                let fid =
                    inner.owner().get_fid_to_endpoint(inner.id.clone().into());

                if let Some(on_start) = inner.on_start.clone() {
                    return Some((
                        on_start,
                        CallbackRequest::new(
                            fid,
                            OnStartEvent {
                                direction: Self::DIRECTION,
                                media_type: started_media_type,
                            },
                            at,
                        ),
                    ));
                }
            }
        }

        None
    }

    /// Returns `true` if `on_start` or `on_stop` callback is set.
    pub fn has_traffic_callback(&self) -> bool {
        let inner = self.0.borrow();
        inner.on_stop.is_some() || inner.on_start.is_some()
    }

    /// Mutes [`MediaType::Audio`] of this [`WebRtcPublishEndpoint`].
    ///
    /// Also this function will call [`WebRtcPlayEndpoint::mute_video`] on all
    /// sinks.
    pub fn mute_audio(&self) {
        let mut inner = self.0.borrow_mut();
        inner.mute_state.started(MediaType::Audio);
        inner
            .sinks()
            .iter()
            .for_each(WebRtcPlayEndpoint::mute_audio);
    }

    /// Unmutes [`MediaType::Audio`] of this [`WebRtcPublishEndpoint`].
    ///
    /// Also this function will call [`WebRtcPlayEndpoint::unmute_video`] on all
    /// sinks.
    pub fn unmute_audio(&self) {
        let mut inner = self.0.borrow_mut();
        inner.mute_state.disable(MediaType::Audio);
        inner
            .sinks()
            .iter()
            .for_each(WebRtcPlayEndpoint::unmute_audio);
    }

    /// Mutes [`MediaType::Video`] of this [`WebRtcPublishEndpoint`].
    ///
    /// Also this function will call [`WebRtcPlayEndpoint::mute_audio`] on all
    /// sinks.
    pub fn mute_video(&self) {
        let mut inner = self.0.borrow_mut();
        inner.mute_state.started(MediaType::Video);
        inner
            .sinks()
            .iter()
            .for_each(WebRtcPlayEndpoint::mute_video);
    }

    /// Unmutes [`MediaType::Video`] of this [`WebRtcPublishEndpoint`].
    ///
    /// Also this function will call [`WebRtcPlayEndpoint::unmute_audio`] on all
    /// sinks.
    pub fn unmute_video(&self) {
        let mut inner = self.0.borrow_mut();
        inner.mute_state.disable(MediaType::Video);
        inner
            .sinks()
            .iter()
            .for_each(WebRtcPlayEndpoint::unmute_video);
    }

    /// Returns [`CallbackUrl`] and [`Fid`] for the `on_stop` Control API
    /// callback of this [`WebRtcPublishEndpoint`].
    ///
    /// Changes [`Peer`] status of this [`WebRtcPublishEndpoint`].
    ///
    /// Sets provided [`MediaType`] to stopped in the
    /// [`WebRtcPublishEndpoint::media_traffic_state`].
    ///
    /// If provided [`MediaType`] will be already stopped then `None` will be
    /// returned.
    pub fn get_on_stop(
        &self,
        peer_id: PeerId,
        at: DateTime<Utc>,
        media_type: MediaType,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        self.set_peer_status(peer_id, false);

        let mut inner = self.0.borrow_mut();
        if !inner.media_traffic_state.is_disabled(media_type) {
            let media_traffic_state_before = inner.media_traffic_state;
            inner.media_traffic_state.disable(media_type);
            let stopped_media_type = get_diff_disabled(
                media_traffic_state_before,
                inner.media_traffic_state,
            )?;
            if inner.mute_state.is_enabled(media_type) {
                let fid =
                    inner.owner().get_fid_to_endpoint(inner.id.clone().into());
                if let Some(url) = inner.on_stop.clone() {
                    return Some((
                        url,
                        CallbackRequest::new(
                            fid,
                            OnStopEvent {
                                reason: OnStopReason::Muted,
                                media_type: stopped_media_type,
                                media_direction: Self::DIRECTION,
                            },
                            at,
                        ),
                    ));
                }
            } else {
                let fid =
                    inner.owner().get_fid_to_endpoint(inner.id.clone().into());
                if let Some(url) = inner.on_stop.clone() {
                    return Some((
                        url,
                        CallbackRequest::new(
                            fid,
                            OnStopEvent {
                                reason: OnStopReason::WrongTrafficFlowing,
                                media_type: stopped_media_type,
                                media_direction: Self::DIRECTION,
                            },
                            at,
                        ),
                    ));
                }
            }
        }

        None
    }

    /// Downgrades [`WebRtcPublishEndpoint`] to weak pointer
    /// [`WeakWebRtcPublishEndpoint`].
    pub fn downgrade(&self) -> WeakWebRtcPublishEndpoint {
        WeakWebRtcPublishEndpoint(Rc::downgrade(&self.0))
    }

    /// Compares [`WebRtcPublishEndpoint`]'s inner pointers. If both pointers
    /// points to the same address, then returns `true`.
    #[cfg(test)]
    pub fn ptr_eq(&self, another_publish: &Self) -> bool {
        Rc::ptr_eq(&self.0, &another_publish.0)
    }
}

/// Weak pointer to [`WebRtcPublishEndpoint`].
#[derive(Clone, Debug)]
pub struct WeakWebRtcPublishEndpoint(Weak<RefCell<WebRtcPublishEndpointInner>>);

impl WeakWebRtcPublishEndpoint {
    /// Upgrades weak pointer to strong pointer.
    ///
    /// # Panics
    ///
    /// If weak pointer was dropped.
    pub fn upgrade(&self) -> WebRtcPublishEndpoint {
        WebRtcPublishEndpoint(self.0.upgrade().unwrap())
    }

    /// Upgrades to [`WebRtcPlayEndpoint`] safely.
    ///
    /// Returns `None` if weak pointer was dropped.
    pub fn safe_upgrade(&self) -> Option<WebRtcPublishEndpoint> {
        self.0.upgrade().map(WebRtcPublishEndpoint)
    }
}

impl Into<proto::WebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn into(self) -> proto::WebRtcPublishEndpoint {
        let p2p: proto::web_rtc_publish_endpoint::P2p = self.p2p().into();
        proto::WebRtcPublishEndpoint {
            p2p: p2p as i32,
            id: self.id().to_string(),
            force_relay: self.is_force_relayed(),
            on_stop: self.0.borrow().on_stop.as_ref().map(ToString::to_string),
            on_start: self
                .0
                .borrow()
                .on_start
                .as_ref()
                .map(ToString::to_string),
        }
    }
}

impl Into<proto::member::Element> for WebRtcPublishEndpoint {
    fn into(self) -> proto::member::Element {
        proto::member::Element {
            el: Some(proto::member::element::El::WebrtcPub(self.into())),
        }
    }
}

impl Into<proto::Element> for WebRtcPublishEndpoint {
    fn into(self) -> proto::Element {
        proto::Element {
            el: Some(proto::element::El::WebrtcPub(self.into())),
        }
    }
}
