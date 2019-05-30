//! Repository that stores [`Room`]s [`Peer`]s.

use hashbrown::HashMap;

use std::convert::{TryFrom, TryInto};

use crate::{
    api::control::{Member, MemberId},
    media::{Peer, PeerId, PeerStateMachine},
    signalling::room::RoomError,
};

#[derive(Debug)]
pub struct PeerRepository {
    /// [`Peer`]s of [`Member`]s in this [`Room`].
    peers: HashMap<PeerId, PeerStateMachine>,

    /// Count of [`Peer`]s in this [`Room`].
    peers_count: u64,

    /// Count of [`MediaTrack`]s in this [`Room`].
    tracks_count: u64,
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

    /// Create and interconnect [`Peer`]s based on [`MemberSpec`].
    ///
    /// Returns IDs of created [`Peer`]s. `(caller_peer_id, responder_peer_id)`.
    pub fn create_peers(
        &mut self,
        caller: &Member,
        responder: &Member,
    ) -> (u64, u64) {
        self.peers_count += 1;
        let caller_peer_id = self.peers_count;
        self.peers_count += 1;
        let responder_peer_id = self.peers_count;

        let mut caller_peer = Peer::new(
            caller_peer_id,
            caller.id.clone(),
            responder_peer_id,
            responder.id.clone(),
        );
        let mut responder_peer = Peer::new(
            responder_peer_id,
            responder.id.clone(),
            caller_peer_id,
            caller.id.clone(),
        );

        caller_peer.add_publish_endpoints(
            caller.spec.get_publish_endpoints(),
            &mut self.tracks_count,
        );
        responder_peer.add_publish_endpoints(
            responder.spec.get_publish_endpoints(),
            &mut self.tracks_count,
        );
        for endpoint in caller.spec.get_play_endpoints() {
            if responder.id == endpoint.src.member_id {
                responder_peer
                    .get_senders()
                    .into_iter()
                    .for_each(|s| caller_peer.add_receiver(s));
            }
        }

        for endpoint in responder.spec.get_play_endpoints() {
            if caller.id == endpoint.src.member_id {
                caller_peer
                    .get_senders()
                    .into_iter()
                    .for_each(|s| responder_peer.add_receiver(s));
            }
        }

        self.add_peer(caller_peer_id, caller_peer);
        self.add_peer(responder_peer_id, responder_peer);

        //        println!("Peers: {:#?}", self.peers);

        (caller_peer_id, responder_peer_id)
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
    #[allow(clippy::ptr_arg)]
    pub fn get_peers_by_member_id(
        &self,
        member_id: &MemberId,
    ) -> Vec<&PeerStateMachine> {
        self.peers
            .iter()
            .filter_map(|(_, peer)| {
                if &peer.member_id() == member_id {
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
        Self {
            peers: map,
            peers_count: 0,
            tracks_count: 0,
        }
    }
}
