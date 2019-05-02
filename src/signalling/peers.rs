//! Repository that stores [`Room`]s [`Peer`]s.
use hashbrown::HashMap;

use std::convert::{TryFrom, TryInto};

use crate::{
    api::control::MemberId,
    media::{Peer, PeerId, PeerStateMachine},
    signalling::room::RoomError,
};

#[derive(Debug)]
pub struct PeerRepository {
    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: HashMap<PeerId, PeerStateMachine>,
}

impl PeerRepository {
    /// Store [`Peer`] in [`Room`].
    pub fn add_peer<S: Into<PeerStateMachine>>(&mut self, id: PeerId, peer: S) {
        self.peers.insert(id, peer.into());
    }

    /// Returns borrowed [`PeerStateMachine`] by its ID.
    pub fn get_peer(
        &self,
        peer_id: PeerId,
    ) -> Result<&PeerStateMachine, RoomError> {
        self.peers
            .get(&peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))
    }

    /// Returns borrowed [`Peer`] by its ID.
    pub fn get_inner_peer<'a, S>(
        &'a self,
        peer_id: PeerId,
    ) -> Result<&'a Peer<S>, RoomError>
    where
        &'a Peer<S>: std::convert::TryFrom<&'a PeerStateMachine>,
        <&'a Peer<S> as TryFrom<&'a PeerStateMachine>>::Error: Into<RoomError>,
    {
        match self.peers.get(&peer_id) {
            Some(peer) => peer.try_into().map_err(Into::into),
            None => Err(RoomError::PeerNotFound(peer_id)),
        }
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
    pub fn take_inner_peer<S>(
        &mut self,
        peer_id: PeerId,
    ) -> Result<Peer<S>, RoomError>
    where
        Peer<S>: TryFrom<PeerStateMachine>,
        <Peer<S> as TryFrom<PeerStateMachine>>::Error: Into<RoomError>,
    {
        match self.peers.remove(&peer_id) {
            Some(peer) => peer.try_into().map_err(Into::into),
            None => Err(RoomError::PeerNotFound(peer_id)),
        }
    }
}

impl From<HashMap<PeerId, PeerStateMachine>> for PeerRepository {
    fn from(map: HashMap<PeerId, PeerStateMachine>) -> Self {
        Self { peers: map }
    }
}
