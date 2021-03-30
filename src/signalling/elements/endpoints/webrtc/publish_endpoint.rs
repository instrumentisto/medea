//! [`WebRtcPublishEndpoint`] implementation.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
};

use medea_client_api_proto::{PeerId, TrackId};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::control::endpoints::webrtc_publish_endpoint::{
        AudioSettings, P2pMode, VideoSettings, WebRtcPublishId as Id,
    },
    signalling::elements::{
        endpoints::webrtc::play_endpoint::WeakWebRtcPlayEndpoint,
        member::WeakMember, Member,
    },
};

use super::play_endpoint::WebRtcPlayEndpoint;

#[derive(Clone, Debug)]
struct WebRtcPublishEndpointInner {
    /// ID of this [`WebRtcPublishEndpoint`].
    id: Id,

    /// [`TrackId`]s of the [`MediaTrack`]s related to this
    /// [`WebRtcPublishEndpoint`].
    tracks_ids: HashMap<PeerId, Vec<TrackId>>,

    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// Indicator whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPublishEndpoint`].
    is_force_relayed: bool,

    /// All sinks of this [`WebRtcPublishEndpoint`].
    sinks: Vec<WeakWebRtcPlayEndpoint>,

    /// Owner [`Member`] of this [`WebRtcPublishEndpoint`].
    owner: WeakMember,

    /// Settings for the audio media type of the [`WebRtcPublishEndpoint`].
    audio_settings: AudioSettings,

    /// Settings for the video media type of the [`WebRtcPublishEndpoint`].
    video_settings: VideoSettings,

    /// [`PeerId`] of all [`Peer`]s created for this [`WebRtcPublishEndpoint`].
    ///
    /// Currently this field used for nothing but in future this may be used
    /// while removing [`WebRtcPublishEndpoint`] for removing all [`Peer`]s of
    /// this [`WebRtcPublishEndpoint`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    peer_ids: HashSet<PeerId>,
}

impl Drop for WebRtcPublishEndpointInner {
    fn drop(&mut self) {
        for receiver in self
            .sinks
            .iter()
            .filter_map(WeakWebRtcPlayEndpoint::safe_upgrade)
        {
            if let Some(receiver_owner) = receiver.weak_owner().safe_upgrade() {
                drop(receiver_owner.remove_sink(&receiver.id()))
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

    fn add_peer_id(&mut self, peer_id: PeerId) {
        self.peer_ids.insert(peer_id);
    }

    fn peer_ids(&self) -> HashSet<PeerId> {
        self.peer_ids.clone()
    }

    fn reset(&mut self) {
        self.peer_ids = HashSet::new()
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.peer_ids.remove(peer_id);
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
    /// Creates new [`WebRtcPublishEndpoint`].
    #[inline]
    #[must_use]
    pub fn new(
        id: Id,
        p2p: P2pMode,
        owner: WeakMember,
        is_force_relayed: bool,
        audio_settings: AudioSettings,
        video_settings: VideoSettings,
    ) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPublishEndpointInner {
            id,
            p2p,
            is_force_relayed,
            sinks: Vec::new(),
            owner,
            audio_settings,
            video_settings,
            peer_ids: HashSet::new(),
            tracks_ids: HashMap::new(),
        })))
    }

    /// Adds [`WebRtcPlayEndpoint`] (sink) to this [`WebRtcPublishEndpoint`].
    #[inline]
    pub fn add_sink(&self, sink: WeakWebRtcPlayEndpoint) {
        self.0.borrow_mut().add_sinks(sink)
    }

    /// Returns all [`WebRtcPlayEndpoint`]s (sinks) of this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// # Panics
    ///
    /// If meets empty pointer.
    #[inline]
    #[must_use]
    pub fn sinks(&self) -> Vec<WebRtcPlayEndpoint> {
        self.0.borrow().sinks()
    }

    /// Returns owner [`Member`] of this [`WebRtcPublishEndpoint`].
    ///
    /// # Panics
    ///
    /// If pointer to [`Member`] has been dropped.
    #[inline]
    #[must_use]
    pub fn owner(&self) -> Member {
        self.0.borrow().owner()
    }

    /// Adds [`PeerId`] of this [`WebRtcPublishEndpoint`].
    #[inline]
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0.borrow_mut().add_peer_id(peer_id)
    }

    /// Returns all [`PeerId`]s of this [`WebRtcPublishEndpoint`].
    #[inline]
    #[must_use]
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.borrow().peer_ids()
    }

    /// Resets state of this [`WebRtcPublishEndpoint`].
    ///
    /// _Atm this only resets `peer_ids`._
    #[inline]
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Removes all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    #[inline]
    pub fn remove_peer_ids(&self, peer_ids: &[PeerId]) {
        self.0.borrow_mut().remove_peer_ids(peer_ids)
    }

    /// Returns [`Id`] of this [`WebRtcPublishEndpoint`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Removes all dropped [`Weak`] pointers from sinks of this
    /// [`WebRtcPublishEndpoint`].
    #[inline]
    pub fn remove_empty_weaks_from_sinks(&self) {
        self.0
            .borrow_mut()
            .sinks
            .retain(|e| e.safe_upgrade().is_some());
    }

    /// Peer-to-peer mode of this [`WebRtcPublishEndpoint`].
    #[inline]
    #[must_use]
    pub fn p2p(&self) -> P2pMode {
        self.0.borrow().p2p
    }

    /// Indicates whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPublishEndpoint`].
    #[inline]
    #[must_use]
    pub fn is_force_relayed(&self) -> bool {
        self.0.borrow().is_force_relayed
    }

    /// Adds [`TrackId`] of the [`MediaTrack`] related to this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// [`MediaTrack`]: crate::media::track::MediaTrack
    #[inline]
    pub fn add_track_id(&self, peer_id: PeerId, track_id: TrackId) {
        let mut inner = self.0.borrow_mut();
        inner.tracks_ids.entry(peer_id).or_default().push(track_id);
    }

    /// Returns [`TrackId`]s of the related to this [`WebRtcPublishEndpoint`]
    /// [`MediaTrack`]s from the [`Peer`] with a provided [`PeerId`].
    ///
    /// [`MediaTrack`]: crate::media::track::MediaTrack
    /// [`Peer`]: crate::media::peer::Peer
    #[inline]
    #[must_use]
    pub fn get_tracks_ids_by_peer_id(&self, peer_id: PeerId) -> Vec<TrackId> {
        let inner = self.0.borrow();
        inner.tracks_ids.get(&peer_id).cloned().unwrap_or_default()
    }

    /// Returns `true` if `on_start` or `on_stop` callback is set.
    #[allow(clippy::unused_self)]
    #[inline]
    #[must_use]
    pub fn has_traffic_callback(&self) -> bool {
        // TODO: Must depend on on_start/on_stop endpoint callbacks, when those
        //       will be added (#91).
        true
    }

    /// Returns [`AudioSettings`] of this [`WebRtcPublishEndpoint`].
    #[inline]
    #[must_use]
    pub fn audio_settings(&self) -> AudioSettings {
        self.0.borrow().audio_settings
    }

    /// Returns [`VideoSettings`] of this [`WebRtcPublishEndpoint`].
    #[inline]
    #[must_use]
    pub fn video_settings(&self) -> VideoSettings {
        self.0.borrow().video_settings
    }

    /// Downgrades [`WebRtcPublishEndpoint`] to weak pointer
    /// [`WeakWebRtcPublishEndpoint`].
    #[inline]
    #[must_use]
    pub fn downgrade(&self) -> WeakWebRtcPublishEndpoint {
        WeakWebRtcPublishEndpoint(Rc::downgrade(&self.0))
    }

    /// Compares [`WebRtcPublishEndpoint`]'s inner pointers. If both pointers
    /// points to the same address, then returns `true`.
    #[cfg(test)]
    #[inline]
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
    #[inline]
    #[must_use]
    pub fn upgrade(&self) -> WebRtcPublishEndpoint {
        WebRtcPublishEndpoint(self.0.upgrade().unwrap())
    }

    /// Upgrades to [`WebRtcPlayEndpoint`] safely.
    ///
    /// Returns [`None`] if weak pointer was dropped.
    #[inline]
    #[must_use]
    pub fn safe_upgrade(&self) -> Option<WebRtcPublishEndpoint> {
        self.0.upgrade().map(WebRtcPublishEndpoint)
    }
}

impl From<WebRtcPublishEndpoint> for proto::WebRtcPublishEndpoint {
    fn from(endpoint: WebRtcPublishEndpoint) -> Self {
        let p2p: proto::web_rtc_publish_endpoint::P2p = endpoint.p2p().into();
        Self {
            p2p: p2p as i32,
            id: endpoint.id().to_string(),
            force_relay: endpoint.is_force_relayed(),
            audio_settings: Some(endpoint.audio_settings().into()),
            video_settings: Some(endpoint.video_settings().into()),
            on_stop: String::new(),
            on_start: String::new(),
        }
    }
}

impl From<WebRtcPublishEndpoint> for proto::member::Element {
    #[inline]
    fn from(endpoint: WebRtcPublishEndpoint) -> Self {
        Self {
            el: Some(proto::member::element::El::WebrtcPub(endpoint.into())),
        }
    }
}

impl From<WebRtcPublishEndpoint> for proto::Element {
    #[inline]
    fn from(endpoint: WebRtcPublishEndpoint) -> Self {
        Self {
            el: Some(proto::element::El::WebrtcPub(endpoint.into())),
        }
    }
}
