use std::{collections::HashMap, rc::Rc};

use super::{PeerConnection, PeerId};

/// [`PeerConnection`] factory and repository.
#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    /// Peer id to [`PeerConnection`],
    peers: HashMap<PeerId, Rc<PeerConnection>>,
}

impl PeerRepository {
    /// Stores [`PeerConnection`] in repository.
    #[inline]
    pub fn insert(
        &mut self,
        id: PeerId,
        peer: Rc<PeerConnection>,
    ) -> Option<Rc<PeerConnection>> {
        self.peers.insert(id, peer)
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    pub fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.peers.get(&id).cloned()
    }

    /// Removes [`PeerConnection`] stored in repository by its ID.
    #[inline]
    pub fn remove(&mut self, id: PeerId) {
        self.peers.remove(&id);
    }

    /// Returns all [`PeerConnection`]s stored in repository.
    #[inline]
    pub fn get_all(&self) -> Vec<Rc<PeerConnection>> {
        self.peers.values().cloned().collect()
    }
}
