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
    /// Returns IDs of created [`Peer`]s. `(first_peer_id, second_peer_id)`.
    pub fn create_peers(
        &mut self,
        first_member: &Member,
        second_member: &Member,
    ) -> (u64, u64) {
        self.peers_count += 1;
        let first_peer_id = self.peers_count;
        self.peers_count += 1;
        let second_peer_id = self.peers_count;

        let mut first_peer = Peer::new(
            first_peer_id,
            first_member.id.clone(),
            second_peer_id,
            second_member.id.clone(),
        );
        let mut second_peer = Peer::new(
            second_peer_id,
            second_member.id.clone(),
            first_peer_id,
            first_member.id.clone(),
        );

        first_peer.add_publish_endpoints(
            first_member.spec.publish_endpoints(),
            &mut self.tracks_count,
        );
        second_peer.add_publish_endpoints(
            second_member.spec.publish_endpoints(),
            &mut self.tracks_count,
        );

        first_peer.add_play_endpoints(
            first_member.spec.play_endpoints(),
            &mut second_peer,
        );
        second_peer.add_play_endpoints(
            second_member.spec.play_endpoints(),
            &mut first_peer,
        );

        self.add_peer(first_peer_id, first_peer);
        self.add_peer(second_peer_id, second_peer);

        //        println!("Peers: {:#?}", self.peers);

        (first_peer_id, second_peer_id)
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

    /// Returns all [`Peer`]s of specified [`Member`].
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

    /// Close all related to disconnected [`Member`] [`Peer`]s and partner
    /// [`Peer`]s.
    pub fn connection_closed(&mut self, member_id: &MemberId) {
        let mut peers_to_remove: Vec<PeerId> = Vec::new();
        for peer in self.get_peers_by_member_id(member_id) {
            for partner_peer in
                self.get_peers_by_member_id(&peer.partner_member_id())
            {
                if &partner_peer.partner_member_id() == member_id {
                    peers_to_remove.push(partner_peer.id());
                }
            }
            peers_to_remove.push(peer.id());
        }

        for peer_id in peers_to_remove {
            self.peers.remove(&peer_id);
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
