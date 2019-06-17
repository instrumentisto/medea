//! Signalling representation of endpoints.

use std::{
    fmt::Display,
    sync::{Arc, Mutex, Weak},
};

use hashbrown::HashSet;

use crate::{
    api::control::endpoint::{P2pMode, SrcUri},
    log::prelude::*,
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

#[derive(Debug, Clone)]
struct WebRtcPlayEndpointInner {
    /// ID of this [`WebRtcPlayEndpoint`].
    id: Id,

    /// Source URI of [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src: SrcUri,

    /// Publisher [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    ///
    /// __Important!__
    ///
    /// As you can see, a __memory leak may occur here__... But it should not
    /// occur beyond the implementation of the [`Drop`] for this
    /// [`WebRtcPlayEndpoint`]. Please be careful and process all future
    /// circular references in the implementation of the [`Drop`] for this
    /// structure.
    publisher: WebRtcPublishEndpoint,

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

    fn owner(&self) -> Weak<Participant> {
        Weak::clone(&self.owner)
    }

    fn publisher(&self) -> WebRtcPublishEndpoint {
        self.publisher.clone()
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

/// Signalling representation of `WebRtcPlayEndpoint`.
///
/// __Important!__
///
/// Please be careful and process all future circular references in the
/// implementation of the [`Drop`] for this structure, otherwise __memory leak
/// may occur here__.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
#[doc(inline)]
pub struct WebRtcPlayEndpoint(Arc<Mutex<WebRtcPlayEndpointInner>>);

impl WebRtcPlayEndpoint {
    /// Create new [`WebRtcPlayEndpoint`].
    pub fn new(
        src: SrcUri,
        publisher: WebRtcPublishEndpoint,
        owner: Weak<Participant>,
        id: Id,
    ) -> Self {
        Self(Arc::new(Mutex::new(WebRtcPlayEndpointInner {
            src,
            publisher,
            owner,
            peer_id: None,
            id,
        })))
    }

    /// Returns [`SrcUri`] of this [`WebRtcPlayEndpoint`].
    pub fn src(&self) -> SrcUri {
        self.0.lock().unwrap().src()
    }

    /// Returns owner [`Participant`] of this [`WebRtcPlayEndpoint`].
    pub fn owner(&self) -> Weak<Participant> {
        self.0.lock().unwrap().owner()
    }

    /// Returns publisher's [`WebRtcPublishEndpoint`].
    pub fn publisher(&self) -> WebRtcPublishEndpoint {
        self.0.lock().unwrap().publisher()
    }

    /// Check that peer connection established for this [`WebRtcPlayEndpoint`].
    pub fn is_connected(&self) -> bool {
        self.0.lock().unwrap().is_connected()
    }

    /// Save [`PeerId`] of this [`WebRtcPlayEndpoint`].
    pub fn connect(&self, peer_id: PeerId) {
        self.0.lock().unwrap().set_peer_id(peer_id);
    }

    /// Return [`PeerId`] of [`Peer`] of this [`WebRtcPlayEndpoint`].
    pub fn peer_id(&self) -> Option<PeerId> {
        self.0.lock().unwrap().peer_id()
    }

    /// Reset state of this [`WebRtcPlayEndpoint`].
    ///
    /// Atm this only reset peer_id.
    pub fn reset(&self) {
        self.0.lock().unwrap().reset()
    }

    pub fn id(&self) -> Id {
        self.0.lock().unwrap().id.clone()
    }
}

impl Drop for WebRtcPlayEndpoint {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) == 1 {
            self.publisher().remove_receiver(&self.id());
        }
    }
}

#[derive(Debug, Clone)]
struct WebRtcPublishEndpointInner {
    /// ID of this [`WebRtcPublishEndpoint`].
    id: Id,

    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// All receivers of this [`WebRtcPublishEndpoint`].
    ///
    /// __Important!__
    ///
    /// As you can see, a __memory leak may occur here__... But it should not
    /// occur beyond the implementation of the [`Drop`] for this
    /// [`WebRtcPublishEndpoint`]. Please be careful and process all future
    /// circular references in the implementation of the [`Drop`] for this
    /// structure.
    receivers: Vec<WebRtcPlayEndpoint>,

    /// Owner [`Participant`] of this [`WebRtcPublishEndpoint`].
    owner: Weak<Participant>,

    /// [`PeerId`] of all [`Peer`]s created for this [`WebRtcPublishEndpoint`].
    ///
    /// Currently this field used for nothing but in future this may be used
    /// while removing [`WebRtcPublishEndpoint`] for removing all [`Peer`]s of
    /// this [`WebRtcPublishEndpoint`].
    peer_ids: HashSet<PeerId>,
}

impl WebRtcPublishEndpointInner {
    fn add_receiver(&mut self, receiver: WebRtcPlayEndpoint) {
        self.receivers.push(receiver);
    }

    fn receivers(&self) -> Vec<WebRtcPlayEndpoint> {
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
///
/// __Important!__
///
/// Please be careful and process all future circular references in the
/// implementation of the [`Drop`] for this structure.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub struct WebRtcPublishEndpoint(Arc<Mutex<WebRtcPublishEndpointInner>>);

impl WebRtcPublishEndpoint {
    /// Create new [`WebRtcPublishEndpoint`].
    pub fn new(
        p2p: P2pMode,
        receivers: Vec<WebRtcPlayEndpoint>,
        owner: Weak<Participant>,
        id: Id,
    ) -> Self {
        Self(Arc::new(Mutex::new(WebRtcPublishEndpointInner {
            p2p,
            receivers,
            owner,
            peer_ids: HashSet::new(),
            id,
        })))
    }

    /// Add receiver for this [`WebRtcPublishEndpoint`].
    pub fn add_receiver(&self, receiver: WebRtcPlayEndpoint) {
        self.0.lock().unwrap().add_receiver(receiver)
    }

    /// Returns all receivers of this [`WebRtcPublishEndpoint`].
    pub fn receivers(&self) -> Vec<WebRtcPlayEndpoint> {
        self.0.lock().unwrap().receivers()
    }

    /// Returns owner [`Participant`] of this [`WebRtcPublishEndpoint`].
    pub fn owner(&self) -> Weak<Participant> {
        self.0.lock().unwrap().owner()
    }

    /// Add [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0.lock().unwrap().add_peer_id(peer_id)
    }

    /// Returns all [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.lock().unwrap().peer_ids()
    }

    /// Reset state of this [`WebRtcPublishEndpoint`].
    ///
    /// Atm this only reset peer_ids.
    pub fn reset(&self) {
        self.0.lock().unwrap().reset()
    }

    /// Remove [`PeerId`] from peer_ids.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn remove_peer_id(&self, peer_id: &PeerId) {
        self.0.lock().unwrap().remove_peer_id(peer_id)
    }

    /// Remove all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    pub fn remove_peer_ids(&self, peer_ids: &[PeerId]) {
        self.0.lock().unwrap().remove_peer_ids(peer_ids)
    }

    /// Returns ID of this [`WebRtcPublishEndpoint`].
    pub fn id(&self) -> Id {
        self.0.lock().unwrap().id.clone()
    }

    /// Remove [`WebRtcPlayEndpoint`] with provided [`Id`] from receivers.
    pub fn remove_receiver(&self, id: &Id) {
        self.0.lock().unwrap().receivers.retain(|e| &e.id() == id);
    }
}

/// This is memory leak fix for [`WebRtcPublishEndpoint`].
impl Drop for WebRtcPublishEndpoint {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) == self.receivers().len() {
            let inner = self.0.lock().unwrap();
            for receiver in &inner.receivers {
                if let Some(receiver_owner) = receiver.owner().upgrade() {
                    receiver_owner.remove_receiver(&receiver.id());
                } else {
                    error!(
                        "Receiver owner for {} WebRtcPublishEndpoint not \
                         found.",
                        inner.id
                    );
                }
            }
        }
    }
}
