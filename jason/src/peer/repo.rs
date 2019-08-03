use std::{collections::HashMap, rc::Rc};

use super::{PeerConnection, PeerId};

#[allow(clippy::module_name_repetitions)]
pub trait PeerRepository {
    /// Stores [`PeerConnection`] in repository.
    fn insert(
        &mut self,
        id: PeerId,
        peer: Rc<PeerConnection>,
    ) -> Option<Rc<PeerConnection>>;

    /// Returns [`PeerConnection`] stored in repository by its ID.
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>>;

    /// Removes [`PeerConnection`] stored in repository by its ID.
    fn remove(&mut self, id: PeerId);

    /// Returns all [`PeerConnection`]s stored in repository.
    fn get_all(&self) -> Vec<Rc<PeerConnection>>;
}

/// [`PeerConnection`] factory and repository.
#[derive(Default)]
pub struct Repository {
    /// Peer id to [`PeerConnection`],
    peers: HashMap<PeerId, Rc<PeerConnection>>,
}

impl PeerRepository for Repository {
    /// Stores [`PeerConnection`] in repository.
    #[inline]
    fn insert(
        &mut self,
        id: PeerId,
        peer: Rc<PeerConnection>,
    ) -> Option<Rc<PeerConnection>> {
        self.peers.insert(id, peer)
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>> {
        self.peers.get(&id).cloned()
    }

    /// Removes [`PeerConnection`] stored in repository by its ID.
    #[inline]
    fn remove(&mut self, id: PeerId) {
        self.peers.remove(&id);
    }

    /// Returns all [`PeerConnection`]s stored in repository.
    #[inline]
    fn get_all(&self) -> Vec<Rc<PeerConnection>> {
        self.peers.values().cloned().collect()
    }
}
