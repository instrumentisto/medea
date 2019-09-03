//! Repository that stores [`Room`]s [`Peer`]s.
//!
//! [`Room`]: crate::signalling::Room
//! [`Peer`]: crate::media::peer::Peer

use std::{
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    fmt,
};

use medea_client_api_proto::{Incrementable, PeerId, TrackId};

use crate::{
    api::control::MemberId,
    log::prelude::*,
    media::{New, Peer, PeerStateMachine},
    signalling::{elements::Member, room::RoomError},
};

#[derive(Debug)]
pub struct PeerRepository {
    /// [`Peer`]s of [`Member`]s in this [`Room`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    /// [`Room`]: crate::signalling::Room
    peers: HashMap<PeerId, PeerStateMachine>,

    /// Count of [`Peer`]s in this [`Room`].
    ///
    /// [`Room`]: crate::signalling::room::Room
    peers_count: Counter<PeerId>,

    /// Count of [`MediaTrack`]s in this [`Room`].
    ///
    /// [`MediaTrack`]: crate::media::track::MediaTrack
    /// [`Room`]: crate::signalling::room::Room
    tracks_count: Counter<TrackId>,
}

/// Simple ID counter.
#[derive(Default, Debug)]
pub struct Counter<T: Incrementable + Copy> {
    count: T,
}

impl<T: Incrementable + Copy> Counter<T> {
    /// Returns id and increase counter.
    pub fn next_id(&mut self) -> T {
        let id = self.count;
        self.count = self.count.increment();

        id
    }
}

impl<T: Incrementable + std::fmt::Display + Copy> fmt::Display for Counter<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.count)
    }
}

impl PeerRepository {
    /// Store [`Peer`] in [`Room`].
    ///
    /// [`Room`]: crate::signalling::Room
    pub fn add_peer<S: Into<PeerStateMachine>>(&mut self, peer: S) {
        let peer = peer.into();
        self.peers.insert(peer.id(), peer);
    }

    /// Returns borrowed [`PeerStateMachine`] by its ID.
    pub fn get_peer_by_id(
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
    pub fn get_tracks_counter(&mut self) -> &mut Counter<TrackId> {
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
        for peer in self.peers.values() {
            if &peer.member_id() == member_id
                && &peer.partner_member_id() == partner_member_id
            {
                return Some((peer.id(), peer.partner_peer_id()));
            }
        }

        None
    }

    /// Returns borrowed [`Peer`] by its ID.
    pub fn get_inner_peer_by_id<'a, S>(
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
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub fn get_peers_by_member_id<'a>(
        &'a self,
        member_id: &'a MemberId,
    ) -> impl Iterator<Item = &'a PeerStateMachine> {
        self.peers.iter().filter_map(move |(_, peer)| {
            if &peer.member_id() == member_id {
                Some(peer)
            } else {
                None
            }
        })
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
    ///
    /// __Note:__ this also delete partner peers.
    pub fn remove_peers(
        &mut self,
        member_id: &MemberId,
        peer_ids: HashSet<PeerId>,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        let mut removed_peers = HashMap::new();
        for peer_id in peer_ids {
            if let Some(peer) = self.peers.remove(&peer_id) {
                let partner_peer_id = peer.partner_peer_id();
                let partner_member_id = peer.partner_member_id();
                if self.peers.remove(&partner_peer_id).is_some() {
                    removed_peers
                        .entry(partner_member_id)
                        .or_insert_with(Vec::new)
                        .push(partner_peer_id);
                }
                removed_peers
                    .entry(member_id.clone())
                    .or_insert_with(Vec::new)
                    .push(peer_id);
            }
        }

        //        for (member_id, removed_peers_ids) in removed_peers {
        //            ctx.notify(PeersRemoved {
        //                member_id,
        //                peers_id: removed_peers_ids,
        //            })
        //        }
        removed_peers
    }

    /// Remove all related to [`Member`] [`Peer`]s.
    /// Note that this function will also remove all partners [`Peer`]s.
    ///
    /// Returns `HashMap` with all remove [`Peer`]s.
    /// Key - [`Peer`]'s owner [`MemberId`],
    /// value - removed [`Peer`]'s [`PeerId`].
    pub fn remove_peers_related_to_member(
        &mut self,
        member_id: &MemberId,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        let mut peers_to_remove: HashMap<MemberId, Vec<PeerId>> =
            HashMap::new();

        self.get_peers_by_member_id(member_id).for_each(|peer| {
            self.get_peers_by_member_id(&peer.partner_member_id())
                .filter(|partner_peer| {
                    &partner_peer.partner_member_id() == member_id
                })
                .for_each(|partner_peer| {
                    peers_to_remove
                        .entry(partner_peer.member_id())
                        .or_insert_with(Vec::new)
                        .push(partner_peer.id());
                });

            peers_to_remove
                .entry(peer.member_id())
                .or_insert_with(Vec::new)
                .push(peer.id());
        });

        peers_to_remove
            .values()
            .flat_map(|peer_ids| peer_ids.iter())
            .for_each(|peer_id| {
                self.peers.remove(peer_id);
            });

        peers_to_remove
    }

    /// Delete [`PeerStateMachine`] from this [`PeerRepository`] and send
    /// [`PeersRemoved`] to [`Member`]s.
    ///
    /// __Note:__ this also delete partner peer.
    pub fn remove_peer(
        &mut self,
        member_id: &MemberId,
        peer_id: PeerId,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        let mut peers_id_to_delete = HashSet::new();
        peers_id_to_delete.insert(peer_id);
        self.remove_peers(&member_id, peers_id_to_delete)
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
