//! [`WebRtcPublishEndpoint`] implementation.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use hashbrown::HashSet;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};

use crate::{
    api::control::endpoint::P2pMode, media::PeerId,
    signalling::elements::Member,
};

use super::play_endpoint::WebRtcPlayEndpoint;

pub use Id as WebRtcPublishId;

pub use crate::api::control::model::endpoint::webrtc::publish_endpoint::Id;

#[derive(Debug, Clone)]
struct WebRtcPublishEndpointInner {
    /// ID of this [`WebRtcPublishEndpoint`].
    id: Id,

    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// All sinks of this [`WebRtcPublishEndpoint`].
    sinks: Vec<Weak<WebRtcPlayEndpoint>>,

    /// Owner [`Member`] of this [`WebRtcPublishEndpoint`].
    owner: Weak<Member>,

    /// [`PeerId`] of all [`Peer`]s created for this [`WebRtcPublishEndpoint`].
    ///
    /// Currently this field used for nothing but in future this may be used
    /// while removing [`WebRtcPublishEndpoint`] for removing all [`Peer`]s of
    /// this [`WebRtcPublishEndpoint`].
    peer_ids: HashSet<PeerId>,
}

impl Drop for WebRtcPublishEndpointInner {
    fn drop(&mut self) {
        for receiver in self.sinks.iter().filter_map(|r| Weak::upgrade(r)) {
            if let Some(receiver_owner) = receiver.weak_owner().upgrade() {
                receiver_owner.remove_sink(&receiver.id())
            }
        }
    }
}

impl WebRtcPublishEndpointInner {
    fn add_sinks(&mut self, sink: Weak<WebRtcPlayEndpoint>) {
        self.sinks.push(sink);
    }

    fn sinks(&self) -> Vec<Rc<WebRtcPlayEndpoint>> {
        self.sinks
            .iter()
            .map(|p| Weak::upgrade(p).unwrap())
            .collect()
    }

    fn owner(&self) -> Rc<Member> {
        Weak::upgrade(&self.owner).unwrap()
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

/// Signalling representation of `WebRtcPublishEndpoint`.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct WebRtcPublishEndpoint(RefCell<WebRtcPublishEndpointInner>);

impl WebRtcPublishEndpoint {
    /// Create new [`WebRtcPublishEndpoint`].
    pub fn new(
        id: Id,
        p2p: P2pMode,
        sinks: Vec<Weak<WebRtcPlayEndpoint>>,
        owner: Weak<Member>,
    ) -> Self {
        Self(RefCell::new(WebRtcPublishEndpointInner {
            id,
            p2p,
            sinks,
            owner,
            peer_ids: HashSet::new(),
        }))
    }

    /// Add sink for this [`WebRtcPublishEndpoint`].
    pub fn add_sink(&self, sink: Weak<WebRtcPlayEndpoint>) {
        self.0.borrow_mut().add_sinks(sink)
    }

    /// Returns all sinks of this [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if meet empty pointer.__
    pub fn sinks(&self) -> Vec<Rc<WebRtcPlayEndpoint>> {
        self.0.borrow().sinks()
    }

    /// Returns owner [`Member`] of this [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if pointer is empty.__
    pub fn owner(&self) -> Rc<Member> {
        self.0.borrow().owner()
    }

    /// Add [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0.borrow_mut().add_peer_id(peer_id)
    }

    /// Returns all [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.borrow().peer_ids()
    }

    /// Reset state of this [`WebRtcPublishEndpoint`].
    ///
    /// Atm this only reset peer_ids.
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Remove [`PeerId`] from peer_ids.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn remove_peer_id(&self, peer_id: &PeerId) {
        self.0.borrow_mut().remove_peer_id(peer_id)
    }

    /// Remove all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    pub fn remove_peer_ids(&self, peer_ids: &[PeerId]) {
        self.0.borrow_mut().remove_peer_ids(peer_ids)
    }

    /// Returns ID of this [`WebRtcPublishEndpoint`].
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Remove all empty Weak pointers from sinks of this
    /// [`WebRtcPublishEndpoint`].
    pub fn remove_empty_weaks_from_sinks(&self) {
        self.0.borrow_mut().sinks.retain(|e| e.upgrade().is_some());
    }
}
