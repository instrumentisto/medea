//! Signalling representation of endpoints.

use std::{
    cell::RefCell,
    sync::{Mutex, Weak},
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

#[derive(Debug, Clone)]
struct WebRtcPlayEndpointInner {
    /// Source URI of [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src: SrcUri,

    /// Publisher [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    publisher: Weak<WebRtcPublishEndpoint>,

    /// Owner [`Participant`] of this [`WebRtcPlayEndpoint`].
    owner: Weak<Participant>,

    /// [`PeerId`] of [`Peer`] created for this [`WebRtcPlayEndpoint`].
    peer_id: Option<PeerId>,
}

impl WebRtcPlayEndpointInner {
    fn src(&self) -> SrcUri {
        self.src.clone()
    }

    fn owner(&self) -> Weak<Participant> {
        Weak::clone(&self.owner)
    }

    fn publisher(&self) -> Weak<WebRtcPublishEndpoint> {
        self.publisher.clone()
    }

    fn is_connected(&self) -> bool {
        self.peer_id.is_some()
    }

    fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = Some(peer_id)
    }

    fn peer_id(&self) -> Option<PeerId> {
        self.peer_id.clone()
    }

    fn reset(&mut self) {
        self.peer_id = None
    }
}

/// Signalling representation of WebRtcPlayEndpoint.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct WebRtcPlayEndpoint(Mutex<RefCell<WebRtcPlayEndpointInner>>);

impl WebRtcPlayEndpoint {
    /// Create new [`WebRtcPlayEndpoint`].
    pub fn new(
        src: SrcUri,
        publisher: Weak<WebRtcPublishEndpoint>,
        owner: Weak<Participant>,
    ) -> Self {
        Self(Mutex::new(RefCell::new(WebRtcPlayEndpointInner {
            src,
            publisher,
            owner,
            peer_id: None,
        })))
    }

    /// Returns [`SrcUri`] of this [`WebRtcPlayEndpoint`].
    pub fn src(&self) -> SrcUri {
        self.0.lock().unwrap().borrow().src()
    }

    /// Returns owner [`Participant`] of this [`WebRtcPlayEndpoint`].
    pub fn owner(&self) -> Weak<Participant> {
        self.0.lock().unwrap().borrow().owner()
    }

    /// Returns publisher's [`WebRtcPublishEndpoint`].
    pub fn publisher(&self) -> Weak<WebRtcPublishEndpoint> {
        self.0.lock().unwrap().borrow().publisher()
    }

    /// Check that peer connection established for this [`WebRtcPlayEndpoint`].
    pub fn is_connected(&self) -> bool {
        self.0.lock().unwrap().borrow().is_connected()
    }

    /// Save [`PeerId`] of this [`WebRtcPlayEndpoint`].
    pub fn connect(&self, peer_id: PeerId) {
        self.0.lock().unwrap().borrow_mut().set_peer_id(peer_id);
    }

    /// Return [`PeerId`] of [`Peer`] of this [`WebRtcPlayEndpoint`].
    pub fn peer_id(&self) -> Option<PeerId> {
        self.0.lock().unwrap().borrow().peer_id()
    }

    /// Reset state of this [`WebRtcPlayEndpoint`].
    ///
    /// Atm this only reset peer_id.
    pub fn reset(&self) {
        self.0.lock().unwrap().borrow_mut().reset()
    }
}

#[derive(Debug, Clone)]
struct WebRtcPublishEndpointInner {
    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// All receivers of this [`WebRtcPublishEndpoint`].
    receivers: Vec<Weak<WebRtcPlayEndpoint>>,

    /// Owner [`Participant`] of this [`WebRtcPublishEndpoint`].
    owner: Weak<Participant>,

    /// All [`PeerId`]s created for this [`WebRtcPublishEndpoint`].
    peer_ids: HashSet<PeerId>,
}

impl WebRtcPublishEndpointInner {
    fn add_receiver(&mut self, receiver: Weak<WebRtcPlayEndpoint>) {
        self.receivers.push(receiver);
    }

    fn receivers(&self) -> Vec<Weak<WebRtcPlayEndpoint>> {
        self.receivers.clone()
    }

    fn owner(&self) -> Weak<Participant> {
        Weak::clone(&self.owner)
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

    fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.peer_ids.remove(peer_id);
    }

    fn remove_peer_ids(&mut self, peer_ids: &Vec<PeerId>) {
        for peer_id in peer_ids {
            self.remove_peer_id(peer_id)
        }
    }
}

/// Signalling representation of WebRtcPublishEndpoint.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct WebRtcPublishEndpoint(Mutex<RefCell<WebRtcPublishEndpointInner>>);

impl WebRtcPublishEndpoint {
    /// Create new [`WebRtcPublishEndpoint`].
    pub fn new(
        p2p: P2pMode,
        receivers: Vec<Weak<WebRtcPlayEndpoint>>,
        owner: Weak<Participant>,
    ) -> Self {
        Self(Mutex::new(RefCell::new(WebRtcPublishEndpointInner {
            p2p,
            receivers,
            owner,
            peer_ids: HashSet::new(),
        })))
    }

    /// Add receiver for this [`WebRtcPublishEndpoint`].
    pub fn add_receiver(&self, receiver: Weak<WebRtcPlayEndpoint>) {
        self.0.lock().unwrap().borrow_mut().add_receiver(receiver)
    }

    /// Returns all receivers of this [`WebRtcPublishEndpoint`].
    pub fn receivers(&self) -> Vec<Weak<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().borrow().receivers()
    }

    /// Returns owner [`Participant`] of this [`WebRtcPublishEndpoint`].
    pub fn owner(&self) -> Weak<Participant> {
        self.0.lock().unwrap().borrow().owner()
    }

    /// Add [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0.lock().unwrap().borrow_mut().add_peer_id(peer_id)
    }

    /// Returns all [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.lock().unwrap().borrow().peer_ids()
    }

    /// Reset state of this [`WebRtcPublishEndpoint`].
    ///
    /// Atm this only reset peer_ids.
    pub fn reset(&self) {
        self.0.lock().unwrap().borrow_mut().reset()
    }

    /// Remove [`PeerId`] from peer_ids.
    pub fn remove_peer_id(&self, peer_id: &PeerId) {
        self.0.lock().unwrap().borrow_mut().remove_peer_id(peer_id)
    }

    /// Remove all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    pub fn remove_peer_ids(&self, peer_ids: &Vec<PeerId>) {
        self.0
            .lock()
            .unwrap()
            .borrow_mut()
            .remove_peer_ids(peer_ids)
    }
}
