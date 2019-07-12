//! Repository that stores [`Room`]s [`Peer`]s.

use std::{
    convert::{TryFrom, TryInto},
    fmt,
};

use actix::{AsyncContext as _, Context};
use hashbrown::{HashMap, HashSet};

use crate::{
    api::control::MemberId,
    log::prelude::*,
    media::{New, Peer, PeerId, PeerStateMachine},
    signalling::{
        elements::Member,
        room::{PeersRemoved, Room, RoomError},
    },
};

#[derive(Debug)]
pub struct PeerRepository {
    /// [`Peer`]s of [`Member`]s in this [`Room`].
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

impl PeerRepository {
    /// Store [`Peer`] in [`Room`].
    pub fn add_peer<S: Into<PeerStateMachine>>(&mut self, peer: S) {
        let peer = peer.into();
        self.peers.insert(peer.id(), peer);
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

    /// Create interconnected [`Peer`]s for provided [`Member`]s.
    pub fn create_peers(
        &mut self,
        first_member: &Member,
        second_member: &Member,
    ) -> (Peer<New>, Peer<New>) {
        debug!(
            "Created peer between {} and {}.",
            first_member.id(),
            second_member.id()
        );
        let first_peer_id = self.peers_count.next_id();
        let second_peer_id = self.peers_count.next_id();

        let first_peer = Peer::new(
            first_peer_id,
            first_member.id().clone(),
            second_peer_id,
            second_member.id().clone(),
        );
        let second_peer = Peer::new(
            second_peer_id,
            second_member.id().clone(),
            first_peer_id,
            first_member.id().clone(),
        );

        (first_peer, second_peer)
    }

    /// Returns mutable reference to track counter.
    pub fn get_tracks_counter(&mut self) -> &mut Counter {
        &mut self.tracks_count
    }

    /// Lookup [`Peer`] of [`Member`] with ID `member_id` which
    /// connected with `partner_member_id`.
    ///
    /// Return Some(peer_id, partner_peer_id) if that [`Peer`] found.
    ///
    /// Return None if that [`Peer`] not found.
    pub fn get_peer_by_members_ids(
        &self,
        member_id: &MemberId,
        partner_member_id: &MemberId,
    ) -> Option<(PeerId, PeerId)> {
        for (_, peer) in &self.peers {
            if &peer.member_id() == member_id
                && &peer.partner_member_id() == partner_member_id
            {
                return Some((peer.id(), peer.partner_peer_id()));
            }
        }

        None
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

    /// Delete [`PeerStateMachine`]s from this [`PeerRepository`] and send
    /// [`PeersRemoved`] to [`Member`]s.
    pub fn remove_peers(
        &mut self,
        member_id: MemberId,
        peer_ids: HashSet<PeerId>,
        ctx: &mut Context<Room>,
    ) {
        let mut removed_peers = HashMap::new();
        for peer_id in peer_ids {
            if let Some(peer) = self.peers.remove(&peer_id) {
                let partner_peer_id = peer.partner_peer_id();
                let partner_member_id = peer.partner_member_id();
                if let Some(_) = self.peers.remove(&partner_peer_id) {
                    removed_peers
                        .entry(partner_member_id)
                        .or_insert(Vec::new())
                        .push(partner_peer_id);
                }
                removed_peers
                    .entry(member_id.clone())
                    .or_insert(Vec::new())
                    .push(peer_id);
            }
        }

        for (member_id, removed_peers_ids) in removed_peers {
            ctx.notify(PeersRemoved {
                member_id,
                peers_id: removed_peers_ids,
            })
        }
    }

    pub fn remove_peer(
        &mut self,
        member_id: MemberId,
        peer_id: PeerId,
        ctx: &mut Context<Room>,
    ) {
        let mut peers_id = HashSet::new();
        peers_id.insert(peer_id);
        self.remove_peers(member_id, peers_id, ctx);
    }

    /// Close all related to disconnected [`Member`] [`Peer`]s and partner
    /// [`Peer`]s.
    ///
    /// Send [`Event::PeersRemoved`] to all affected [`Member`]s.
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
