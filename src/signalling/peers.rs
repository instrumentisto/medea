//! Repository that stores [`Room`]s [`Peer`]s.
//!
//! [`Room`]: crate::signalling::Room
//! [`Peer`]: crate::media::peer::Peer

use std::{
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    sync::Arc,
};

use actix::{fut::wrap_future, ActorFuture};
use derive_more::Display;
use medea_client_api_proto::{Incrementable, PeerId, TrackId};

use crate::{
    api::control::{MemberId, RoomId},
    log::prelude::*,
    media::{peer::Stable, Peer, PeerError, PeerStateMachine},
    signalling::{
        elements::endpoints::webrtc::{
            WebRtcPlayEndpoint, WebRtcPublishEndpoint,
        },
        room::{ActFuture, RoomError},
        Room,
    },
    turn::{TurnAuthService, UnreachablePolicy},
};

#[derive(Debug)]
pub struct PeerRepository {
    /// [`RoomId`] of the [`Room`] which owns this [`PeerRepository`].
    room_id: RoomId,

    /// [`TurnAuthService`] that [`IceUser`]s for the [`PeerConnection`]s from
    /// this [`PeerRepository`] will be created with.
    turn_service: Arc<dyn TurnAuthService>,

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
#[derive(Default, Debug, Clone, Copy, Display)]
pub struct Counter<T> {
    count: T,
}

impl<T: Incrementable + Copy> Counter<T> {
    /// Returns id and increase counter.
    pub fn next_id(&mut self) -> T {
        let id = self.count;
        self.count = self.count.incr();
        id
    }
}

impl PeerRepository {
    /// Returns new [`PeerRepository`] for a [`Room`] with the provided
    /// [`RoomId`].
    pub fn new(
        room_id: RoomId,
        turn_service: Arc<dyn TurnAuthService>,
    ) -> Self {
        Self {
            room_id,
            turn_service,
            peers: HashMap::new(),
            peers_count: Counter::default(),
            tracks_count: Counter::default(),
        }
    }

    /// Store [`Peer`] in [`Room`].
    ///
    /// [`Room`]: crate::signalling::Room
    pub fn add_peer<S: Into<PeerStateMachine>>(&mut self, peer: S) {
        let peer = peer.into();
        self.peers.insert(peer.id(), peer);
    }

    /// Returns borrowed [`PeerStateMachine`] by its ID.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub fn get_peer_by_id(
        &self,
        peer_id: PeerId,
    ) -> Result<&PeerStateMachine, RoomError> {
        self.peers
            .get(&peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))
    }

    /// Returns mutably borrowed [`PeerStateMachine`] by its ID.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub fn get_mut_peer_by_id(
        &mut self,
        peer_id: PeerId,
    ) -> Result<&mut PeerStateMachine, RoomError> {
        self.peers
            .get_mut(&peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))
    }

    /// Creates interconnected [`Peer`]s for provided [`Member`]s.
    pub fn create_peers(
        &mut self,
        src: &WebRtcPublishEndpoint,
        sink: &WebRtcPlayEndpoint,
    ) -> (Peer<Stable>, Peer<Stable>) {
        let src_member_id = src.owner().id();
        let sink_member_id = sink.owner().id();

        debug!(
            "Created peer between {} and {}.",
            src_member_id, sink_member_id
        );
        let src_peer_id = self.peers_count.next_id();
        let sink_peer_id = self.peers_count.next_id();

        let first_peer = Peer::new(
            src_peer_id,
            src_member_id.clone(),
            sink_peer_id,
            sink_member_id.clone(),
            src.is_force_relayed(),
        );
        let second_peer = Peer::new(
            sink_peer_id,
            sink_member_id,
            src_peer_id,
            src_member_id,
            sink.is_force_relayed(),
        );

        (first_peer, second_peer)
    }

    /// Returns mutable reference to track counter.
    pub fn get_tracks_counter(&mut self) -> &mut Counter<TrackId> {
        &mut self.tracks_count
    }

    /// Lookups [`Peer`] of [`Member`] with ID `member_id` which
    /// connected with `partner_member_id`.
    ///
    /// Returns `Some(peer_id, partner_peer_id)` if [`Peer`] has been found,
    /// otherwise returns `None`.
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
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
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
        self.peers
            .values()
            .filter(move |peer| &peer.member_id() == member_id)
    }

    /// Returns owned [`Peer`] by its ID. If [`Peer`] is found, but it is in
    /// unexcpected state, then it is returned to repository.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    ///
    /// Errors with [`RoomError::PeerError`] if [`Peer`] is found, but not in
    /// requested state.
    pub fn take_inner_peer<S>(
        &mut self,
        peer_id: PeerId,
    ) -> Result<Peer<S>, RoomError>
    where
        Peer<S>: TryFrom<PeerStateMachine>,
        <Peer<S> as TryFrom<PeerStateMachine>>::Error:
            Into<(PeerError, PeerStateMachine)>,
    {
        match self.peers.remove(&peer_id) {
            Some(peer) => match peer.try_into() {
                Ok(peer) => Ok(peer),
                Err(err) => {
                    let (err, peer) = err.into();
                    self.peers.insert(peer_id, peer);
                    Err(RoomError::from(err))
                }
            },
            None => Err(RoomError::PeerNotFound(peer_id)),
        }
    }

    /// Deletes [`PeerStateMachine`]s from this [`PeerRepository`] and send
    /// [`Event::PeersRemoved`] to [`Member`]s.
    ///
    /// __Note:__ this also deletes partner peers.
    ///
    /// [`Event::PeersRemoved`]: medea_client_api_proto::Event::PeersRemoved
    pub fn remove_peers(
        &mut self,
        member_id: &MemberId,
        peer_ids: &HashSet<PeerId>,
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
                    .push(*peer_id);
            }
        }

        removed_peers
    }

    /// Removes all [`Peer`]s related to given [`Member`].
    /// Note, that this function will also remove all partners [`Peer`]s.
    ///
    /// Returns `HashMap` with all removed [`Peer`]s.
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

    /// Deletes [`PeerStateMachine`] from this [`PeerRepository`] and send
    /// [`Event::PeersRemoved`] to [`Member`]s.
    ///
    /// __Note:__ this also deletes partner peer.
    ///
    /// [`Event::PeersRemoved`]: medea_client_api_proto::Event::PeersRemoved
    pub fn remove_peer(
        &mut self,
        member_id: &MemberId,
        peer_id: PeerId,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        let mut peers_id_to_delete = HashSet::new();
        peers_id_to_delete.insert(peer_id);
        self.remove_peers(&member_id, &peers_id_to_delete)
    }

    /// Creates [`Peer`] for endpoints if [`Peer`] between endpoint's members
    /// doesn't exist.
    ///
    /// Adds `send` track to source member's [`Peer`] and `recv` to
    /// sink member's [`Peer`]. Registers TURN credentials for created
    /// [`Peer`]s.
    ///
    /// Returns [`PeerId`]s of newly created [`Peer`] if it has been created.
    ///
    /// # Errors
    ///
    /// Errors if could not save [`IceUser`] in [`TurnAuthService`].
    ///
    /// # Panics
    ///
    /// Panics if provided endpoints already have interconnected [`Peer`]s.
    pub fn connect_endpoints(
        &mut self,
        src: &WebRtcPublishEndpoint,
        sink: &WebRtcPlayEndpoint,
    ) -> ActFuture<Result<Option<(PeerId, PeerId)>, RoomError>> {
        debug!(
            "Connecting endpoints of Member [id = {}] with Member [id = {}]",
            src.owner().id(),
            sink.owner().id(),
        );
        let src_owner = src.owner();
        let sink_owner = sink.owner();

        if let Some((src_peer_id, sink_peer_id)) =
            self.get_peer_by_members_ids(&src_owner.id(), &sink_owner.id())
        {
            // TODO: when dynamic patching of [`Room`] will be done then we need
            //       rewrite this code to updating [`Peer`]s in not
            //       [`Peer<Stable>`] state.
            let mut src_peer: Peer<Stable> =
                self.take_inner_peer(src_peer_id).unwrap();
            let mut sink_peer: Peer<Stable> =
                self.take_inner_peer(sink_peer_id).unwrap();

            src_peer.add_publisher(&mut sink_peer, self.get_tracks_counter());

            src.add_peer_id(src_peer_id);
            sink.set_peer_id(sink_peer_id);

            self.add_peer(src_peer);
            self.add_peer(sink_peer);

            Box::new(actix::fut::ready(Ok(None)))
        } else {
            let (mut src_peer, mut sink_peer) = self.create_peers(&src, &sink);

            src_peer.add_publisher(&mut sink_peer, self.get_tracks_counter());

            src.add_peer_id(src_peer.id());
            sink.set_peer_id(sink_peer.id());

            let src_peer_id = src_peer.id();
            let sink_peer_id = sink_peer.id();

            self.add_peer(src_peer);
            self.add_peer(sink_peer);

            let room_id = self.room_id.clone();
            let turn_service = Arc::clone(&self.turn_service);
            Box::new(
                wrap_future(async move {
                    let src_ice_user = turn_service.create(
                        room_id.clone(),
                        src_peer_id,
                        UnreachablePolicy::ReturnErr,
                    );
                    let sink_ice_user = turn_service.create(
                        room_id,
                        sink_peer_id,
                        UnreachablePolicy::ReturnErr,
                    );
                    Ok(futures::try_join!(src_ice_user, sink_ice_user)?)
                })
                .then(move |result, room: &mut Room, _| {
                    match result {
                        Ok((src_ice_user, sink_ice_user)) => {
                            match room.peers.get_mut_peer_by_id(src_peer_id) {
                                Ok(src_peer) => {
                                    src_peer.set_ice_user(src_ice_user);
                                }
                                Err(err) => {
                                    return actix::fut::err(err);
                                }
                            };
                            match room.peers.get_mut_peer_by_id(sink_peer_id) {
                                Ok(sink_peer) => {
                                    sink_peer.set_ice_user(sink_ice_user);
                                }
                                Err(err) => {
                                    return actix::fut::err(err);
                                }
                            };
                            actix::fut::ok(Some((src_peer_id, sink_peer_id)))
                        }
                        Err(err) => actix::fut::err(err),
                    }
                }),
            )
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;

    use crate::{media::WaitRemoteSdp, turn::new_turn_auth_service_mock};

    #[test]
    fn take_inner_peer() {
        let peer = Peer::<Stable>::new(
            PeerId(1),
            MemberId::from("member"),
            PeerId(2),
            MemberId::from("partner_member"),
            false,
        );

        let mut peers = HashMap::new();
        peers.insert(peer.id(), peer.into());

        let mut repo = PeerRepository {
            room_id: RoomId::from("w/e"),
            turn_service: new_turn_auth_service_mock(),
            peers,
            peers_count: Counter::default(),
            tracks_count: Counter::default(),
        };

        // bad id
        match repo.take_inner_peer::<Stable>(PeerId(2)) {
            Ok(_) => unreachable!(),
            Err(err) => match err {
                RoomError::PeerNotFound(peer_id) => {
                    assert_eq!(peer_id, PeerId(2));
                }
                _ => unreachable!(),
            },
        }
        // peer still there
        assert!(repo.get_peer_by_id(PeerId(1)).is_ok());

        // bad state
        if let Err(RoomError::PeerError(PeerError::WrongState(..))) =
            repo.take_inner_peer::<WaitRemoteSdp>(PeerId(1))
        {
            // ok
        } else {
            unreachable!()
        }
        // peer still there
        assert!(repo.get_peer_by_id(PeerId(1)).is_ok());

        // ok
        let peer = repo.take_inner_peer::<Stable>(PeerId(1)).unwrap();
        assert_eq!(peer.id(), PeerId(1));
        // peer is taken
        assert!(repo.get_peer_by_id(PeerId(1)).is_err())
    }
}
