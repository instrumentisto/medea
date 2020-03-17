//! Repository that stores [`Room`]s [`Peer`]s.
//!
//! [`Room`]: crate::signalling::Room
//! [`Peer`]: crate::media::peer::Peer

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    future::Future,
    rc::Rc,
    sync::Arc,
};

use derive_more::Display;
use medea_client_api_proto::{Incrementable, PeerId, TrackId};

use crate::{
    api::control::{MemberId, RoomId},
    log::prelude::*,
    media::{IceUser, New, Peer, PeerStateMachine},
    signalling::{
        elements::endpoints::{
            webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            WeakEndpoint,
        },
        room::RoomError,
    },
    turn::{TurnAuthService, UnreachablePolicy},
};

#[derive(Debug)]
pub struct PeerRepository {
    room_id: RoomId,

    ice_users: Rc<RefCell<HashMap<PeerId, IceUser>>>,

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

    peers_endpoints: HashMap<PeerId, Vec<WeakEndpoint>>,
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
    pub fn new(
        room_id: RoomId,
        turn_service: Arc<dyn TurnAuthService>,
    ) -> Self {
        Self {
            room_id,
            turn_service,
            ice_users: Rc::new(RefCell::new(HashMap::new())),
            peers: HashMap::new(),
            peers_count: Counter::default(),
            tracks_count: Counter::default(),
            peers_endpoints: HashMap::new(),
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
    pub fn get_peer_by_id_mut(
        &mut self,
        peer_id: PeerId,
    ) -> Result<&mut PeerStateMachine, RoomError> {
        self.peers
            .get_mut(&peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))
    }

    pub fn get_ice_user(
        &mut self,
        peer_id: PeerId,
    ) -> impl Future<Output = IceUser> {
        let ice_users = Rc::clone(&self.ice_users);
        let turn_service = Arc::clone(&self.turn_service);
        let room_id = self.room_id.clone();

        async move {
            let ice_user = ice_users.borrow().get(&peer_id).cloned();
            if let Some(ice_user) = ice_user {
                ice_user
            } else {
                let ice_user = turn_service
                    .create(room_id, peer_id, UnreachablePolicy::ReturnErr)
                    .await
                    .unwrap();
                ice_users.borrow_mut().insert(peer_id, ice_user.clone());

                ice_user
            }
        }
    }

    /// Creates interconnected [`Peer`]s for provided [`Member`]s.
    pub fn create_peers(
        &mut self,
        src: &WebRtcPublishEndpoint,
        sink: &WebRtcPlayEndpoint,
    ) -> (Peer<New>, Peer<New>) {
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

    /// Returns owned [`Peer`] by its ID.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
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
    /// sink member's [`Peer`].
    ///
    /// Returns [`PeerId`]s of newly created [`Peer`] if it has been created.
    ///
    /// # Panics
    ///
    /// Panics if provided endpoints have interconnected [`Peer`]s already.
    pub fn connect_endpoints(
        &mut self,
        src: &WebRtcPublishEndpoint,
        sink: &WebRtcPlayEndpoint,
    ) -> Option<(PeerId, PeerId)> {
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
            //       [`Peer<New>`] state.
            let mut src_peer: Peer<New> =
                self.take_inner_peer(src_peer_id).unwrap();
            let mut sink_peer: Peer<New> =
                self.take_inner_peer(sink_peer_id).unwrap();

            src_peer.add_publisher(&mut sink_peer, self.get_tracks_counter());

            src.add_peer_id(src_peer_id);
            self.peers_endpoints
                .entry(src_peer_id)
                .or_default()
                .push(src.downgrade().into());
            sink.set_peer_id(sink_peer_id);
            self.peers_endpoints
                .entry(sink_peer_id)
                .or_default()
                .push(sink.downgrade().into());

            self.add_peer(src_peer);
            self.add_peer(sink_peer);
        } else {
            let (mut src_peer, mut sink_peer) = self.create_peers(&src, &sink);

            src_peer.add_publisher(&mut sink_peer, self.get_tracks_counter());

            src.add_peer_id(src_peer.id());
            self.peers_endpoints
                .entry(src_peer.id())
                .or_default()
                .push(src.downgrade().into());
            sink.set_peer_id(sink_peer.id());
            self.peers_endpoints
                .entry(sink_peer.id())
                .or_default()
                .push(sink.downgrade().into());

            let src_peer_id = src_peer.id();
            let sink_peer_id = sink_peer.id();

            self.add_peer(src_peer);
            self.add_peer(sink_peer);

            return Some((src_peer_id, sink_peer_id));
        };

        None
    }

    pub fn get_endpoint_path_by_peer_id(
        &self,
        peer_id: PeerId,
    ) -> Option<Vec<WeakEndpoint>> {
        self.peers_endpoints.get(&peer_id).cloned()
    }
}
