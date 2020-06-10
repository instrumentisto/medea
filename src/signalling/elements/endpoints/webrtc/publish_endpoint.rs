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
        P2pMode, WebRtcPublishId as Id,
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

    tracks_ids: HashMap<PeerId, HashSet<TrackId>>,

    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// Indicator whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPublishEndpoint`].
    is_force_relayed: bool,

    /// All sinks of this [`WebRtcPublishEndpoint`].
    sinks: Vec<WeakWebRtcPlayEndpoint>,

    /// Owner [`Member`] of this [`WebRtcPublishEndpoint`].
    owner: WeakMember,

    /// [`PeerId`] of all [`Peer`]s created for this [`WebRtcPublishEndpoint`].
    ///
    /// Currently this field used for nothing but in future this may be used
    /// while removing [`WebRtcPublishEndpoint`] for removing all [`Peer`]s of
    /// this [`WebRtcPublishEndpoint`].
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
    pub fn new(
        id: Id,
        p2p: P2pMode,
        owner: WeakMember,
        is_force_relayed: bool,
    ) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPublishEndpointInner {
            id,
            p2p,
            is_force_relayed,
            sinks: Vec::new(),
            owner,
            peer_ids: HashSet::new(),
            tracks_ids: HashMap::new(),
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

    /// Adds [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0.borrow_mut().add_peer_id(peer_id)
    }

    /// Returns all [`PeerId`]s of this [`WebRtcPublishEndpoint`].
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.borrow().peer_ids()
    }

    /// Resets state of this [`WebRtcPublishEndpoint`].
    ///
    /// _Atm this only resets `peer_ids`._
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Removes [`PeerId`] from this [`WebRtcPublishEndpoint`]'s `peer_ids`.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn remove_peer_id(&self, peer_id: &PeerId) {
        self.0.borrow_mut().remove_peer_id(peer_id)
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

    pub fn add_track_id(&self, peer_id: PeerId, track_id: TrackId) {
        let mut inner = self.0.borrow_mut();
        inner
            .tracks_ids
            .entry(peer_id)
            .or_insert_with(|| HashSet::new())
            .insert(track_id);
    }

    pub fn get_tracks_ids_by_peer_id(
        &self,
        peer_id: PeerId,
    ) -> HashSet<TrackId> {
        let inner = self.0.borrow();
        inner
            .tracks_ids
            .get(&peer_id)
            .cloned()
            .unwrap_or_else(|| HashSet::new())
    }

    pub fn get_all_tracks_ids(&self) -> HashMap<PeerId, HashSet<TrackId>> {
        let inner = self.0.borrow();
        inner.tracks_ids.clone()
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
            on_stop: String::new(),
            on_start: String::new(),
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
