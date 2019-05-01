//! Repository that stores [`Room`]s [`Peer`]s.
use hashbrown::HashMap;

use crate::{
    api::control::MemberId,
    media::{PeerId, PeerStateMachine},
    signalling::room::RoomError,
};

#[derive(Debug)]
pub struct PeerRepository {
    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: HashMap<PeerId, PeerStateMachine>,
}

impl PeerRepository {
    /// Store [`Peer`] in [`Room`].
    pub fn add_peer(&mut self, id: PeerId, peer: PeerStateMachine) {
        self.peers.insert(id, peer);
    }

    /// Returns borrowed [`Peer`] by its ID.
    pub fn get_peer(
        &self,
        peer_id: PeerId,
    ) -> Result<&PeerStateMachine, RoomError> {
        self.peers
            .get(&peer_id)
            .ok_or_else(|| RoomError::UnknownPeer(peer_id))
    }

    /// Returns [`Peer`] of specified [`Member`].
    ///
    /// Panic if [`Peer`] not exists.
    pub fn get_peers_by_member_id(
        &self,
        member_id: MemberId,
    ) -> Vec<&PeerStateMachine> {
        self.peers
            .iter()
            .filter_map(|(_, peer)| {
                if peer.member_id() == member_id {
                    Some(peer)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns owned [`Peer`] by its ID.
    pub fn take_peer(
        &mut self,
        peer_id: PeerId,
    ) -> Result<PeerStateMachine, RoomError> {
        self.peers
            .remove(&peer_id)
            .ok_or_else(|| RoomError::UnknownPeer(peer_id))
    }
}

impl From<HashMap<PeerId, PeerStateMachine>> for PeerRepository {
    fn from(map: HashMap<PeerId, PeerStateMachine>) -> Self {
        Self { peers: map }
    }
}
