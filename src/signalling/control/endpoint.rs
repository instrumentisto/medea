//! Signalling representation of endpoints.

use std::{
    cell::RefCell,
    fmt::Display,
    rc::{Rc, Weak},
};

use hashbrown::HashSet;

use crate::{
    api::control::endpoint::{P2pMode, SrcUri},
    media::PeerId,
};

use super::participant::Participant;

/// ID of endpoint.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub String);

impl Display for Id {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", self.0)
    }
}

impl From<String> for Id {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone)]
struct WebRtcPlayEndpointInner {
    /// ID of this [`WebRtcPlayEndpoint`].
    id: Id,

    /// Source URI of [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src: SrcUri,

    /// Publisher [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    publisher: Weak<WebRtcPublishEndpoint>,

    /// Owner [`Participant`] of this [`WebRtcPlayEndpoint`].
    owner: Weak<Participant>,

    /// [`PeerId`] of [`Peer`] created for this [`WebRtcPlayEndpoint`].
    ///
    /// Currently this field used for detecting status of this
    /// [`WebRtcPlayEndpoint`] connection.
    ///
    /// In future this may be used for removing [`WebRtcPlayEndpoint`]
    /// and related peer.
    peer_id: Option<PeerId>,
}

impl WebRtcPlayEndpointInner {
    fn src(&self) -> SrcUri {
        self.src.clone()
    }

    fn owner(&self) -> Rc<Participant> {
        Weak::upgrade(&self.owner).unwrap()
    }

    fn weak_owner(&self) -> Weak<Participant> {
        Weak::clone(&self.owner)
    }

    fn publisher(&self) -> Rc<WebRtcPublishEndpoint> {
        Weak::upgrade(&self.publisher).unwrap()
    }

    fn is_connected(&self) -> bool {
        self.peer_id.is_some()
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
        if let Some(receiver_publisher) = self.publisher.upgrade() {
            receiver_publisher.remove_empty_weaks_from_receivers();
        }
    }
}

/// Signalling representation of `WebRtcPlayEndpoint`.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct WebRtcPlayEndpoint(RefCell<WebRtcPlayEndpointInner>);

impl WebRtcPlayEndpoint {
    /// Create new [`WebRtcPlayEndpoint`].
    pub fn new(
        id: Id,
        src: SrcUri,
        publisher: Weak<WebRtcPublishEndpoint>,
        owner: Weak<Participant>,
    ) -> Self {
        Self(RefCell::new(WebRtcPlayEndpointInner {
            id,
            src,
            publisher,
            owner,
            peer_id: None,
        }))
    }

    /// Returns [`SrcUri`] of this [`WebRtcPlayEndpoint`].
    pub fn src(&self) -> SrcUri {
        self.0.borrow().src()
    }

    /// Returns owner [`Participant`] of this [`WebRtcPlayEndpoint`].
    pub fn owner(&self) -> Rc<Participant> {
        self.0.borrow().owner()
    }

    // TODO: explain this
    pub fn weak_owner(&self) -> Weak<Participant> {
        self.0.borrow().weak_owner()
    }

    /// Returns publisher's [`WebRtcPublishEndpoint`].
    pub fn publisher(&self) -> Rc<WebRtcPublishEndpoint> {
        self.0.borrow().publisher()
    }

    /// Check that peer connection established for this [`WebRtcPlayEndpoint`].
    pub fn is_connected(&self) -> bool {
        self.0.borrow().is_connected()
    }

    /// Save [`PeerId`] of this [`WebRtcPlayEndpoint`].
    pub fn connect(&self, peer_id: PeerId) {
        self.0.borrow_mut().set_peer_id(peer_id);
    }

    /// Return [`PeerId`] of [`Peer`] of this [`WebRtcPlayEndpoint`].
    pub fn peer_id(&self) -> Option<PeerId> {
        self.0.borrow().peer_id()
    }

    /// Reset state of this [`WebRtcPlayEndpoint`].
    ///
    /// Atm this only reset peer_id.
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Returns ID of this [`WebRtcPlayEndpoint`].
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }
}

#[derive(Debug, Clone)]
struct WebRtcPublishEndpointInner {
    /// ID of this [`WebRtcPublishEndpoint`].
    id: Id,

    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// All receivers of this [`WebRtcPublishEndpoint`].
    receivers: Vec<Weak<WebRtcPlayEndpoint>>,

    /// Owner [`Participant`] of this [`WebRtcPublishEndpoint`].
    owner: Weak<Participant>,

    /// [`PeerId`] of all [`Peer`]s created for this [`WebRtcPublishEndpoint`].
    ///
    /// Currently this field used for nothing but in future this may be used
    /// while removing [`WebRtcPublishEndpoint`] for removing all [`Peer`]s of
    /// this [`WebRtcPublishEndpoint`].
    peer_ids: HashSet<PeerId>,
}

impl Drop for WebRtcPublishEndpointInner {
    fn drop(&mut self) {
        // TODO: add comments
        for receiver in self.receivers.iter().filter_map(|r| Weak::upgrade(r)) {
            if let Some(receiver_owner) = receiver.weak_owner().upgrade() {
                receiver_owner.remove_receiver(&receiver.id())
            }
        }
    }
}

impl WebRtcPublishEndpointInner {
    fn add_receiver(&mut self, receiver: Weak<WebRtcPlayEndpoint>) {
        self.receivers.push(receiver);
    }

    fn receivers(&self) -> Vec<Rc<WebRtcPlayEndpoint>> {
        self.receivers
            .iter()
            .map(|p| Weak::upgrade(p).unwrap())
            .collect()
    }

    fn owner(&self) -> Rc<Participant> {
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
        receivers: Vec<Weak<WebRtcPlayEndpoint>>,
        owner: Weak<Participant>,
    ) -> Self {
        Self(RefCell::new(WebRtcPublishEndpointInner {
            id,
            p2p,
            receivers,
            owner,
            peer_ids: HashSet::new(),
        }))
    }

    /// Add receiver for this [`WebRtcPublishEndpoint`].
    pub fn add_receiver(&self, receiver: Weak<WebRtcPlayEndpoint>) {
        self.0.borrow_mut().add_receiver(receiver)
    }

    /// Returns all receivers of this [`WebRtcPublishEndpoint`].
    pub fn receivers(&self) -> Vec<Rc<WebRtcPlayEndpoint>> {
        self.0.borrow().receivers()
    }

    /// Returns owner [`Participant`] of this [`WebRtcPublishEndpoint`].
    pub fn owner(&self) -> Rc<Participant> {
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

    /// Remove all empty Weak pointers from receivers of this
    /// [`WebRtcPublishEndpoint`].
    pub fn remove_empty_weaks_from_receivers(&self) {
        self.0
            .borrow_mut()
            .receivers
            .retain(|e| e.upgrade().is_some());
    }
}
