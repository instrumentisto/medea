//! Repository that stores [`Room`]s [`Peer`]s.

use std::{
    convert::{TryFrom, TryInto},
    fmt,
};

use actix::{AsyncContext as _, Context};
use hashbrown::HashMap;

use crate::{
    api::control::MemberId,
    media::{Peer, PeerId, PeerStateMachine},
    signalling::{
        control::participant::Participant,
        room::{PeersRemoved, Room, RoomError},
    },
};

#[derive(Debug)]
pub struct PeerRepository {
    /// [`Peer`]s of [`Participant`]s in this [`Room`].
    peers: HashMap<PeerId, PeerStateMachine>,

    /// Count of [`Peer`]s in this [`Room`].
    peers_count: Counter,

    /// Count of [`MediaTrack`]s in this [`Room`].
    tracks_count: Counter,
}

/// Simple ID counter.
#[derive(Default, Debug)]
pub struct Counter {
    count: u64,
}

impl Counter {
    /// Returns id and increase counter.
    pub fn next_id(&mut self) -> u64 {
        let id = self.count;
        self.count += 1;

        id
    }
}

impl fmt::Display for Counter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.count)
    }
}

use crate::log::prelude::*;

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

    /// Create and interconnect [`Peer`]s based on [`Participant`].
    ///
    /// Returns IDs of created [`Peer`]s. `(first_peer_id, second_peer_id)`.
    pub fn create_peers(
        &mut self,
        first_member: &Participant,
        second_member: &Participant,
    ) -> (u64, u64) {
        debug!(
            "Created peer between {} and {}.",
            first_member.id(),
            second_member.id()
        );
        let first_peer_id = self.peers_count.next_id();
        let second_peer_id = self.peers_count.next_id();

        let mut first_peer = Peer::new(
            first_peer_id,
            first_member.id().clone(),
            second_peer_id,
            second_member.id().clone(),
        );
        let mut second_peer = Peer::new(
            second_peer_id,
            second_member.id().clone(),
            first_peer_id,
            first_member.id().clone(),
        );

        first_peer.add_publish_endpoints(
            &mut second_peer,
            &mut self.tracks_count,
            first_member.publishers(),
        );
        second_peer.add_publish_endpoints(
            &mut first_peer,
            &mut self.tracks_count,
            second_member.publishers(),
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

    /// Returns all [`Peer`]s of specified [`Participant`].
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

    /// Close all related to disconnected [`Participant`] [`Peer`]s and partner
    /// [`Peer`]s.
    ///
    /// Send [`Event::PeersRemoved`] to all affected [`Participant`]s.
    pub fn connection_closed(
        &mut self,
        member_id: &MemberId,
        ctx: &mut Context<Room>,
    ) {
        let mut peers_to_remove: HashMap<MemberId, Vec<PeerId>> =
            HashMap::new();

        self.get_peers_by_member_id(member_id)
            .into_iter()
            .for_each(|peer| {
                self.get_peers_by_member_id(&peer.partner_member_id())
                    .into_iter()
                    .filter(|partner_peer| {
                        &partner_peer.partner_member_id() == member_id
                    })
                    .for_each(|partner_peer| {
                        peers_to_remove
                            .entry(partner_peer.member_id())
                            .or_insert(Vec::new())
                            .push(partner_peer.id());
                    });

                peers_to_remove
                    .entry(peer.partner_member_id())
                    .or_insert(Vec::new())
                    .push(peer.id());

                peers_to_remove
                    .entry(peer.member_id())
                    .or_insert(Vec::new())
                    .push(peer.id());
            });

        for (peer_member_id, peers_id) in peers_to_remove {
            for peer_id in &peers_id {
                self.peers.remove(peer_id);
            }
            ctx.notify(PeersRemoved {
                member_id: peer_member_id,
                peers_id,
            })
        }
    }
}

impl From<HashMap<PeerId, PeerStateMachine>> for PeerRepository {
    fn from(map: HashMap<PeerId, PeerStateMachine>) -> Self {
        Self {
            peers: map,
            peers_count: Counter::default(),
            tracks_count: Counter::default(),
        }
    }
}
