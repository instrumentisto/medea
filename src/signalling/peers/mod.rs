//! Repository that stores [`Room`]s [`Peer`]s.
//!
//! [`Peer`]: crate::media::peer::Peer
//! [`Room`]: crate::signalling::room::Room

mod media_traffic_state;
mod metrics;
mod traffic_watcher;

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto},
    rc::Rc,
    sync::Arc,
};

use derive_more::Display;
use futures::{future, Stream};
use medea_client_api_proto::{
    state, stats::RtcStat, Incrementable, MemberId, PeerConnectionState,
    PeerId, RoomId, TrackId,
};

use crate::{
    conf,
    log::prelude::*,
    media::{peer::PeerUpdatesSubscriber, Peer, PeerError, PeerStateMachine},
    signalling::{
        elements::endpoints::{
            webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            Endpoint,
        },
        peers::metrics::{PeerMetricsService, RtcStatsHandler},
        room::RoomError,
    },
    turn::{TurnAuthService, UnreachablePolicy},
};

pub use self::{
    metrics::{PeersMetricsEvent, PeersMetricsEventHandler},
    traffic_watcher::{
        build_peers_traffic_watcher, FlowMetricSource,
        PeerConnectionStateEventsHandler, PeerTrafficWatcher,
    },
};

#[derive(Debug)]
pub struct PeersService {
    /// [`RoomId`] of the [`Room`] which owns this [`PeerRepository`].
    ///
    /// [`Room`]: crate::signalling::room::Room
    room_id: RoomId,

    /// [`TurnAuthService`] that [`IceUser`]s for the [`Peer`]s from
    /// this [`PeerRepository`] will be created with.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    /// [`IceUser`]: crate::turn::IceUser
    turn_service: Arc<dyn TurnAuthService>,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    /// [`Peer`]: crate::media::peer::Peer
    /// [`Room`]: crate::signalling::Room
    peers: PeerRepository,

    /// Count of [`Peer`]s in this [`Room`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    /// [`Room`]: crate::signalling::room::Room
    peers_count: Counter<PeerId>,

    /// Count of [`MediaTrack`]s in this [`Room`].
    ///
    /// [`MediaTrack`]: crate::media::track::MediaTrack
    /// [`Room`]: crate::signalling::room::Room
    tracks_count: Counter<TrackId>,

    /// [`PeerTrafficWatcher`] which analyzes [`Peer`]s traffic metrics.
    /// [`Peer`]: crate::media::peer::Peer
    peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,

    /// Service which responsible for this [`Room`]'s [`RtcStat`]s processing.
    ///
    /// [`Room`]: crate::signalling::room::Room
    peer_metrics_service: RefCell<Box<dyn RtcStatsHandler>>,

    /// Subscriber to the events which indicates that negotiation process
    /// should be started for a some [`Peer`].
    negotiation_sub: Rc<dyn PeerUpdatesSubscriber>,
}

/// Simple ID counter.
#[derive(Clone, Debug, Default, Display)]
pub struct Counter<T: Copy> {
    count: Cell<T>,
}

impl<T: Incrementable + Copy> Counter<T> {
    /// Returns id and increase counter.
    pub fn next_id(&self) -> T {
        let id = self.count.get();
        self.count.set(id.incr());
        id
    }
}

/// Result of the [`PeersService::get_or_create_peers`] function.
#[derive(Clone, Copy, Debug)]
enum GetOrCreatePeersResult {
    /// Requested [`Peer`] pair was created.
    Created(PeerId, PeerId),

    /// Requested [`Peer`] pair already existed.
    AlreadyExisted(PeerId, PeerId),
}

/// All changes which can be performed on a [`Peer`].
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum PeerChange {
    /// [`Peer`] was removed from a [`PeersService`].
    Removed(MemberId, PeerId),

    /// [`Peer`] was updated and renegotiation for this [`Peer`] should be
    /// performed.
    Updated(PeerId),
}

impl PeersService {
    /// Returns new [`PeerRepository`] for a [`Room`] with the provided
    /// [`RoomId`].
    ///
    /// [`Room`]: crate::signalling::room::Room
    pub fn new(
        room_id: RoomId,
        turn_service: Arc<dyn TurnAuthService>,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
        media_conf: &conf::Media,
        negotiation_sub: Rc<dyn PeerUpdatesSubscriber>,
    ) -> Rc<Self> {
        Rc::new(Self {
            room_id: room_id.clone(),
            turn_service,
            peers: PeerRepository::default(),
            peers_count: Counter::default(),
            tracks_count: Counter::default(),
            peers_traffic_watcher: Arc::clone(&peers_traffic_watcher),
            peer_metrics_service: RefCell::new(Box::new(
                PeerMetricsService::new(
                    room_id,
                    peers_traffic_watcher,
                    media_conf.max_lag,
                ),
            )),
            negotiation_sub,
        })
    }

    /// Store [`Peer`] in [`Room`].
    ///
    /// [`Room`]: crate::signalling::Room
    #[inline]
    pub fn add_peer<S: Into<PeerStateMachine>>(&self, peer: S) {
        self.peers.add_peer(peer)
    }

    /// Applies a function to the [`PeerStateMachine`] reference with provided
    /// [`PeerId`] (if any found).
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    #[inline]
    pub fn map_peer_by_id<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        self.peers.map_peer_by_id(peer_id, f)
    }

    /// Applies a function to the mutable [`PeerStateMachine`] reference with
    /// provided [`PeerId`] (if any found).
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    #[inline]
    pub fn map_peer_by_id_mut<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&mut PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        self.peers.map_peer_by_id_mut(peer_id, f)
    }

    /// Creates interconnected [`Peer`]s for provided endpoints and saves them
    /// in [`PeersService`].
    ///
    /// Returns [`PeerId`]s of the created [`Peer`]s.
    fn create_peers(
        &self,
        src: &WebRtcPublishEndpoint,
        sink: &WebRtcPlayEndpoint,
    ) -> (PeerId, PeerId) {
        let src_member_id = src.owner().id();
        let sink_member_id = sink.owner().id();

        let src_peer_id = self.peers_count.next_id();
        let sink_peer_id = self.peers_count.next_id();

        debug!(
            "Created peers:[{}, {}] between {} and {}.",
            src_peer_id, sink_peer_id, src_member_id, sink_member_id,
        );

        let mut src_peer = PeerStateMachine::from(Peer::new(
            src_peer_id,
            src_member_id.clone(),
            sink_peer_id,
            sink_member_id.clone(),
            src.is_force_relayed(),
            Rc::clone(&self.negotiation_sub),
        ));
        src_peer.add_endpoint(&src.clone().into());

        let mut sink_peer = PeerStateMachine::from(Peer::new(
            sink_peer_id,
            sink_member_id,
            src_peer_id,
            src_member_id,
            sink.is_force_relayed(),
            Rc::clone(&self.negotiation_sub),
        ));
        sink_peer.add_endpoint(&sink.clone().into());

        src_peer.as_changes_scheduler().add_publisher(
            &src,
            &mut sink_peer,
            &self.tracks_count,
        );

        self.peer_metrics_service
            .borrow_mut()
            .register_peer(&src_peer);
        self.peer_metrics_service
            .borrow_mut()
            .register_peer(&sink_peer);

        self.add_peer(src_peer);
        self.add_peer(sink_peer);

        (src_peer_id, sink_peer_id)
    }

    /// Lookups [`Peer`] of [`Member`] with ID `member_id` which
    /// connected with `partner_member_id`.
    ///
    /// Returns `Some(peer_id, partner_peer_id)` if [`Peer`] has been found,
    /// otherwise returns `None`.
    ///
    /// [`Member`]: crate::signalling::elements::Member
    #[inline]
    pub fn get_peers_between_members(
        &self,
        member_id: &MemberId,
        partner_member_id: &MemberId,
    ) -> Option<(PeerId, PeerId)> {
        self.peers
            .get_peers_between_members(member_id, partner_member_id)
    }

    /// Returns owned [`Peer`] by its ID.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    ///
    /// Errors with [`RoomError::PeerError`] if [`Peer`] is found, but not in
    /// requested state.
    pub fn take_inner_peer<S>(
        &self,
        peer_id: PeerId,
    ) -> Result<Peer<S>, RoomError>
    where
        Peer<S>: TryFrom<PeerStateMachine>,
        <Peer<S> as TryFrom<PeerStateMachine>>::Error:
            Into<(PeerError, PeerStateMachine)>,
    {
        self.peers.take_inner_peer(peer_id)
    }

    /// Deletes [`PeerStateMachine`]s from this [`PeerRepository`] and send
    /// [`Event::PeersRemoved`] to [`Member`]s.
    ///
    /// __Note:__ this also deletes partner peers.
    ///
    /// [`Event::PeersRemoved`]: medea_client_api_proto::Event::PeersRemoved
    /// [`Member`]: crate::signalling::elements::Member
    pub fn remove_peers<'a, Peers: IntoIterator<Item = &'a PeerId>>(
        &self,
        member_id: &MemberId,
        peer_ids: Peers,
    ) -> HashMap<MemberId, Vec<PeerStateMachine>> {
        let mut removed_peers = HashMap::new();
        for peer_id in peer_ids {
            if let Some(peer) = self.peers.remove(*peer_id) {
                let partner_peer_id = peer.partner_peer_id();
                let partner_member_id = peer.partner_member_id();
                if let Some(partner_peer) = self.peers.remove(partner_peer_id) {
                    removed_peers
                        .entry(partner_member_id.clone())
                        .or_insert_with(Vec::new)
                        .push(partner_peer);
                }
                removed_peers
                    .entry(member_id.clone())
                    .or_insert_with(Vec::new)
                    .push(peer);
            }
        }

        let peers_to_unregister: Vec<_> = removed_peers
            .values()
            .flat_map(|peer| peer.iter().map(PeerStateMachine::id))
            .collect();
        self.peer_metrics_service
            .borrow_mut()
            .unregister_peers(&peers_to_unregister);
        self.peers_traffic_watcher
            .unregister_peers(self.room_id.clone(), peers_to_unregister);

        removed_peers
    }

    /// Deletes the provided [`WebRtcPlayEndpoint`].
    ///
    /// Returns [`PeerChange`]s which were performed by this function.
    #[inline]
    pub fn delete_sink_endpoint(
        &self,
        sink: &WebRtcPlayEndpoint,
    ) -> HashSet<PeerChange> {
        if let Ok(change) = self.peers.delete_sink_endpoint(sink) {
            change
        } else {
            // This can happen only if the provided endpoint contains peers that
            // don't exist anymore. Not a reason to propagate error, since we're
            // removing this endpoint anyway, but that means that we didn't
            // clean this endpoint up.
            warn!("Error while removing sink {:?}", sink);
            HashSet::new()
        }
    }

    /// Deletes the provided [`WebRtcPublishEndpoint`].
    ///
    /// Returns [`PeerChange`]s which were performed by this function.
    ///
    /// # Errors
    ///
    /// If a [`Peer`] with the provided [`PeerId`] or a partner [`Peer`] hasn't
    /// been found.
    #[inline]
    pub fn delete_src_endpoint(
        &self,
        src: &WebRtcPublishEndpoint,
    ) -> HashSet<PeerChange> {
        if let Ok(change) = self.peers.delete_src_endpoint(src) {
            change
        } else {
            // This can happen only if the provided endpoint contains peers that
            // don't exist anymore. Not a reason to propagate error, since we're
            // removing this endpoint anyway, but that means that we didn't
            // clean this endpoint up.
            warn!("Error while removing sink {:?}", src);
            HashSet::new()
        }
    }

    /// Returns already created [`Peer`] pair's [`PeerId`]s as
    /// [`GetOrCreatePeersResult::AlreadyExisted`] variant.
    ///
    /// Returns newly created [`Peer`] pair's [`PeerId`]s as
    /// [`GetOrCreatePeersResult::Created`] variant.
    async fn get_or_create_peers(
        &self,
        src: &WebRtcPublishEndpoint,
        sink: &WebRtcPlayEndpoint,
    ) -> Result<GetOrCreatePeersResult, RoomError> {
        if let Some((first_peer_id, second_peer_id)) = self
            .get_peers_between_members(&src.owner().id(), &sink.owner().id())
        {
            Ok(GetOrCreatePeersResult::AlreadyExisted(
                first_peer_id,
                second_peer_id,
            ))
        } else {
            let (src_peer_id, sink_peer_id) = self.create_peers(&src, &sink);

            self.peer_post_construct(src_peer_id, &src.clone().into())
                .await?;
            self.peer_post_construct(sink_peer_id, &sink.clone().into())
                .await?;

            Ok(GetOrCreatePeersResult::Created(src_peer_id, sink_peer_id))
        }
    }

    /// Tries to run all scheduled changes on specified [`Peer`] and its partner
    /// [`Peer`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub fn commit_scheduled_changes(
        &self,
        peer_id: PeerId,
    ) -> Result<(), RoomError> {
        let partner_peer_id =
            self.peers.map_peer_by_id_mut(peer_id, |peer| {
                peer.commit_scheduled_changes();
                peer.partner_peer_id()
            })?;

        self.peers.map_peer_by_id_mut(partner_peer_id, |peer| {
            peer.commit_scheduled_changes();
        })?;

        Ok(())
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
    /// [`IceUser`]: crate::turn::IceUser
    pub async fn connect_endpoints(
        self: Rc<Self>,
        src: WebRtcPublishEndpoint,
        sink: WebRtcPlayEndpoint,
    ) -> Result<Option<(PeerId, PeerId)>, RoomError> {
        debug!(
            "Connecting endpoints of Member [id = {}] with Member [id = {}]",
            src.owner().id(),
            sink.owner().id(),
        );
        match self.get_or_create_peers(&src, &sink).await? {
            GetOrCreatePeersResult::Created(src_peer_id, sink_peer_id) => {
                Ok(Some((src_peer_id, sink_peer_id)))
            }
            GetOrCreatePeersResult::AlreadyExisted(
                src_peer_id,
                sink_peer_id,
            ) => {
                if sink.peer_id().is_some()
                    || src.peer_ids().contains(&src_peer_id)
                {
                    // already connected, so no-op
                    Ok(None)
                } else {
                    let mut src_peer = self.peers.take(src_peer_id)?;
                    let mut sink_peer = self.peers.take(sink_peer_id)?;

                    src_peer.as_changes_scheduler().add_publisher(
                        &src,
                        &mut sink_peer,
                        &self.tracks_count,
                    );

                    let mut register_peer_tasks = Vec::new();
                    if src.has_traffic_callback() {
                        register_peer_tasks.push(
                            self.peers_traffic_watcher.register_peer(
                                self.room_id.clone(),
                                src_peer_id,
                                src.is_force_relayed(),
                            ),
                        );
                    }
                    if sink.has_traffic_callback() {
                        register_peer_tasks.push(
                            self.peers_traffic_watcher.register_peer(
                                self.room_id.clone(),
                                sink_peer_id,
                                sink.is_force_relayed(),
                            ),
                        );
                    }

                    sink_peer.add_endpoint(&sink.into());
                    src_peer.add_endpoint(&src.into());

                    self.peers.add_peer(src_peer);
                    self.peers.add_peer(sink_peer);

                    future::try_join_all(register_peer_tasks)
                        .await
                        .map_err(RoomError::PeerTrafficWatcherMailbox)?;

                    Ok(Some((src_peer_id, sink_peer_id)))
                }
            }
        }
    }

    /// Creates and sets [`IceUser`], registers [`Peer`] in
    /// [`PeerTrafficWatcher`].
    ///
    /// [`IceUser`]: crate::turn::ice_user::IceUser
    async fn peer_post_construct(
        &self,
        peer_id: PeerId,
        endpoint: &Endpoint,
    ) -> Result<(), RoomError> {
        let ice_users = self
            .turn_service
            .create(self.room_id.clone(), peer_id, UnreachablePolicy::ReturnErr)
            .await?;

        self.peers.map_peer_by_id_mut(peer_id, move |p| {
            p.add_ice_users(ice_users);
            p.set_initialized();
        })?;

        if endpoint.has_traffic_callback() {
            self.peers_traffic_watcher
                .register_peer(
                    self.room_id.clone(),
                    peer_id,
                    endpoint.is_force_relayed(),
                )
                .await
                .map_err(RoomError::PeerTrafficWatcherMailbox)
        } else {
            Ok(())
        }
    }

    /// Updates [`PeerMetricsService`] tracks of the [`Peer`] with provided
    /// [`PeerId`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub(super) fn update_peer_tracks(
        &self,
        peer_id: PeerId,
    ) -> Result<(), RoomError> {
        self.peers.map_peer_by_id(peer_id, |peer| {
            self.peer_metrics_service.borrow_mut().update_peer(peer);
        })?;

        Ok(())
    }

    /// Removes all [`Peer`]s related to given [`Member`].
    /// Note, that this function will also remove all partners [`Peer`]s.
    ///
    /// Returns [`HashMap`] with all removed [`Peer`]s:
    /// key - [`Peer`]'s owner [`MemberId`],
    /// value - removed [`Peer`]'s [`PeerId`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    // TODO: remove in #91.
    pub(super) fn remove_peers_related_to_member(
        &self,
        member_id: &MemberId,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        let peers = self.peers.remove_peers_related_to_member(member_id);

        if !peers.is_empty() {
            let all_peers: Vec<PeerId> =
                peers.values().flatten().copied().collect();
            self.peer_metrics_service
                .borrow_mut()
                .unregister_peers(&all_peers);
            self.peers_traffic_watcher
                .unregister_peers(self.room_id.clone(), all_peers);
        }

        peers
    }

    /// Updates tracks information of the [`Peer`] with provided [`PeerId`] in
    /// the [`RtcStatsHandler`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub(super) fn sync_peer_spec(
        &self,
        peer_id: PeerId,
    ) -> Result<(), RoomError> {
        self.peers.map_peer_by_id(peer_id, |peer| {
            self.peer_metrics_service.borrow_mut().update_peer(&peer);
        })?;
        Ok(())
    }

    /// Returns [`Stream`] of [`PeersMetricsEvent`]s from underlying
    /// [`RtcStatsHandler`].
    pub(super) fn subscribe_to_metrics_events(
        &self,
    ) -> impl Stream<Item = PeersMetricsEvent> {
        self.peer_metrics_service.borrow_mut().subscribe()
    }

    /// Propagates stats to [`RtcStatsHandler`].
    pub(super) fn add_stats(&self, peer_id: PeerId, stats: &[RtcStat]) {
        self.peer_metrics_service
            .borrow_mut()
            .add_stats(peer_id, stats);
    }

    /// Propagates [`PeerConnectionState`] to [`RtcStatsHandler`].
    pub(super) fn update_peer_connection_state(
        &self,
        peer_id: PeerId,
        state: PeerConnectionState,
    ) {
        self.peer_metrics_service
            .borrow_mut()
            .update_peer_connection_state(peer_id, state);
    }

    /// Runs [`Peer`]s stats checking in the underlying [`PeersMetricsEvent`]s.
    pub(super) fn check_peers(&self) {
        self.peer_metrics_service.borrow_mut().check();
    }

    /// Returns [`state::Peer`]s for all [`Peer`]s owned by the provided
    /// [`MemberId`].
    #[inline]
    #[must_use]
    pub(super) fn get_peers_states(
        &self,
        member_id: &MemberId,
    ) -> HashMap<PeerId, state::Peer> {
        self.peers.get_peers_states(member_id)
    }
}

/// Repository which stores all [`PeerStateMachine`]s of the [`PeersService`].
#[derive(Debug, Default)]
pub struct PeerRepository(RefCell<HashMap<PeerId, PeerStateMachine>>);

impl PeerRepository {
    /// Applies a function to the [`PeerStateMachine`] reference with provided
    /// [`PeerId`] (if any found).
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    fn map_peer_by_id<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        Ok(f(self
            .0
            .borrow()
            .get(&peer_id)
            .ok_or(RoomError::PeerNotFound(peer_id))?))
    }

    /// Applies a function to the mutable [`PeerStateMachine`] reference with
    /// provided [`PeerId`] (if any found).
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    fn map_peer_by_id_mut<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&mut PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        Ok(f(self
            .0
            .borrow_mut()
            .get_mut(&peer_id)
            .ok_or(RoomError::PeerNotFound(peer_id))?))
    }

    /// Removes [`PeerStateMachine`] with a provided [`PeerId`].
    ///
    /// Returns removed [`PeerStateMachine`] if it existed.
    fn remove(&self, peer_id: PeerId) -> Option<PeerStateMachine> {
        self.0.borrow_mut().remove(&peer_id)
    }

    /// Removes [`PeerStateMachine`] with a provided [`PeerId`] and returns
    /// removed [`PeerStateMachine`] if it existed.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    fn take(&self, peer_id: PeerId) -> Result<PeerStateMachine, RoomError> {
        self.remove(peer_id).ok_or(RoomError::PeerNotFound(peer_id))
    }

    /// Returns owned [`Peer`] by its ID.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    ///
    /// Errors with [`RoomError::PeerError`] if [`Peer`] is found, but not in
    /// requested state.
    fn take_inner_peer<S>(&self, peer_id: PeerId) -> Result<Peer<S>, RoomError>
    where
        Peer<S>: TryFrom<PeerStateMachine>,
        <Peer<S> as TryFrom<PeerStateMachine>>::Error:
            Into<(PeerError, PeerStateMachine)>,
    {
        type Err<S> = <Peer<S> as TryFrom<PeerStateMachine>>::Error;

        self.take(peer_id)?.try_into().map_err(|e: Err<S>| {
            let (err, peer) = e.into();
            self.add_peer(peer);
            RoomError::from(err)
        })
    }

    /// Stores [`Peer`] in [`Room`].
    ///
    /// [`Room`]: crate::signalling::Room
    fn add_peer<S: Into<PeerStateMachine>>(&self, peer: S) {
        let peer = peer.into();
        self.0.borrow_mut().insert(peer.id(), peer);
    }

    /// Lookups [`Peer`] of [`Member`] with ID `member_id` which
    /// connected with `partner_member_id`.
    ///
    /// Returns `Some(peer_id, partner_peer_id)` if [`Peer`] has been found,
    /// otherwise returns `None`.
    ///
    /// [`Member`]: crate::signalling::elements::Member
    fn get_peers_between_members(
        &self,
        member_id: &MemberId,
        partner_member_id: &MemberId,
    ) -> Option<(PeerId, PeerId)> {
        for peer in self.0.borrow().values() {
            if peer.member_id() == member_id
                && peer.partner_member_id() == partner_member_id
            {
                return Some((peer.id(), peer.partner_peer_id()));
            }
        }

        None
    }

    /// Deletes the provided [`WebRtcPublishEndpoint`].
    ///
    /// Returns [`PeerChange`]s which were performed by this action.
    ///
    /// # Errors
    ///
    /// With [`RoomError::PeerNotFound`] if the requested [`PeerId`] doesn't
    /// exist in a [`PeerRepository`].
    pub fn delete_src_endpoint(
        &self,
        src: &WebRtcPublishEndpoint,
    ) -> Result<HashSet<PeerChange>, RoomError> {
        let mut affected_peers = HashSet::new();
        for sink in src.sinks() {
            affected_peers.extend(self.delete_sink_endpoint(&sink)?);
        }

        Ok(affected_peers)
    }

    /// Deletes the provided [`WebRtcPlayEndpoint`].
    ///
    /// Returns [`PeerChange`]s which were performed by this action.
    ///
    /// # Errors
    ///
    /// With [`RoomError::PeerNotFound`] if the requested [`PeerId`] doesn't
    /// exist in a [`PeerRepository`].
    pub fn delete_sink_endpoint(
        &self,
        sink_endpoint: &WebRtcPlayEndpoint,
    ) -> Result<HashSet<PeerChange>, RoomError> {
        let mut changes = HashSet::new();

        if let Some(sink_peer_id) = sink_endpoint.peer_id() {
            let (src_peer_id, tracks_to_remove) =
                self.map_peer_by_id_mut(sink_peer_id, |sink_peer| {
                    let src_peer_id = sink_peer.partner_peer_id();
                    let src_endpoint = sink_endpoint.src();
                    let tracks_to_remove =
                        src_endpoint.get_tracks_ids_by_peer_id(src_peer_id);
                    sink_peer
                        .as_changes_scheduler()
                        .remove_tracks(&tracks_to_remove);

                    (src_peer_id, tracks_to_remove)
                })?;
            self.map_peer_by_id_mut(src_peer_id, |src_peer| {
                src_peer
                    .as_changes_scheduler()
                    .remove_tracks(&tracks_to_remove);
            })?;

            let is_sink_peer_empty =
                self.map_peer_by_id(sink_peer_id, PeerStateMachine::is_empty)?;
            let is_src_peer_empty =
                self.map_peer_by_id(src_peer_id, PeerStateMachine::is_empty)?;

            if is_sink_peer_empty && is_src_peer_empty {
                let member = sink_endpoint.owner();
                member.peers_removed(&[sink_peer_id]);

                self.remove(sink_peer_id);
                self.remove(src_peer_id);

                changes.insert(PeerChange::Removed(member.id(), sink_peer_id));
                changes.insert(PeerChange::Removed(
                    sink_endpoint.src().owner().id(),
                    src_peer_id,
                ));
            } else {
                changes.insert(PeerChange::Updated(sink_peer_id));
            }
        }

        Ok(changes)
    }

    /// Removes all [`Peer`]s related to given [`Member`].
    /// Note, that this function will also remove all partners [`Peer`]s.
    ///
    /// Returns [`HashMap`] with all removed [`Peer`]s:
    /// key - [`Peer`]'s owner [`MemberId`],
    /// value - removed [`Peer`]'s [`PeerId`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    // TODO: remove in #91.
    fn remove_peers_related_to_member(
        &self,
        member_id: &MemberId,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        let mut peers_to_remove: HashMap<MemberId, Vec<PeerId>> =
            HashMap::new();

        self.0
            .borrow()
            .values()
            .filter(|p| p.member_id() == member_id)
            .for_each(|peer| {
                self.0
                    .borrow()
                    .values()
                    .filter(|p| p.member_id() == peer.partner_member_id())
                    .filter(|partner_peer| {
                        partner_peer.partner_member_id() == member_id
                    })
                    .for_each(|partner_peer| {
                        peers_to_remove
                            .entry(partner_peer.member_id().clone())
                            .or_default()
                            .push(partner_peer.id());
                    });

                peers_to_remove
                    .entry(peer.member_id().clone())
                    .or_default()
                    .push(peer.id());
            });

        peers_to_remove
            .values()
            .flat_map(|p| p.iter())
            .for_each(|id| {
                self.0.borrow_mut().remove(id);
            });

        peers_to_remove
    }

    /// Returns [`state::Peer`]s for all [`Peer`]s owned by the provided
    /// [`MemberId`].
    #[must_use]
    pub fn get_peers_states(
        &self,
        member_id: &MemberId,
    ) -> HashMap<PeerId, state::Peer> {
        self.0
            .borrow()
            .iter()
            .filter_map(|(id, p)| {
                if p.member_id() == member_id
                    && (p.is_known_to_remote()
                        || p.negotiation_role().is_some())
                {
                    Some((*id, p.get_state()))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, time::Duration};

    use futures::{channel::mpsc, future, Stream, StreamExt as _};
    use medea_client_api_proto::PeerUpdate;
    use tokio::time::timeout;

    use crate::{
        api::control::{
            endpoints::webrtc_publish_endpoint::{
                AudioSettings, P2pMode, VideoSettings,
            },
            member::Credential,
            refs::SrcUri,
        },
        signalling::{
            elements::Member, peers::traffic_watcher::MockPeerTrafficWatcher,
        },
        turn::test::new_turn_auth_service_mock,
    };

    use super::{metrics::MockRtcStatsHandler, *};

    impl PeersService {
        fn with_metrics_service(
            room_id: RoomId,
            turn_service: Arc<dyn TurnAuthService>,
            peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
            negotiation_sub: Rc<dyn PeerUpdatesSubscriber>,
            peer_metrics_service: Box<dyn RtcStatsHandler>,
        ) -> Rc<Self> {
            Rc::new(Self {
                room_id,
                turn_service,
                peers: PeerRepository::default(),
                peers_count: Counter::default(),
                tracks_count: Counter::default(),
                peers_traffic_watcher,
                peer_metrics_service: RefCell::new(peer_metrics_service),
                negotiation_sub,
            })
        }
    }

    /// Mock for the [`PeerUpdatesSubscriber`] trait.
    ///
    /// You can subscribe to the [`Stream`] into which will be sent all
    /// [`PeerId`]s of [`Peer`] which are should be renegotiated.
    #[derive(Debug, Clone)]
    struct NegotiationSubMock(
        Rc<RefCell<Vec<mpsc::UnboundedSender<PeerId>>>>,
        Rc<RefCell<Vec<mpsc::UnboundedSender<(PeerId, Vec<PeerUpdate>)>>>>,
    );

    impl NegotiationSubMock {
        /// Returns new empty [`NegotiationSubMock`].
        pub fn new() -> Self {
            Self(Rc::default(), Rc::default())
        }

        /// Returns [`Stream`] into which will be sent all [`PeerId`]s of
        /// [`Peer`] which are should be renegotiated.
        pub fn on_negotiation_needed(&self) -> impl Stream<Item = PeerId> {
            let (tx, rx) = mpsc::unbounded();
            self.0.borrow_mut().push(tx);
            rx
        }

        /// Returns [`Stream`] into which will be sent all [`PeerId`]s and
        /// [`PeerUpdate`]s of [`Peer`] which are should be forcibly updated.
        #[allow(dead_code)]
        pub fn on_force_update(
            &self,
        ) -> impl Stream<Item = (PeerId, Vec<PeerUpdate>)> {
            let (tx, rx) = mpsc::unbounded();
            self.1.borrow_mut().push(tx);
            rx
        }
    }

    impl PeerUpdatesSubscriber for NegotiationSubMock {
        /// Sends [`PeerId`] to the
        /// [`NegotiationSubMock::on_negotiation_needed`] [`Stream`].
        fn negotiation_needed(&self, peer_id: PeerId) {
            self.0.borrow().iter().for_each(|sender| {
                let _ = sender.unbounded_send(peer_id);
            });
        }

        /// Sends [`PeerId`] to the [`NegotiationSubMock::on_force_update`]
        /// [`Stream`].
        fn force_update(&self, peer_id: PeerId, changes: Vec<PeerUpdate>) {
            self.1.borrow().iter().for_each(|sender| {
                let _ = sender.unbounded_send((peer_id, changes.clone()));
            })
        }
    }

    /// Returns [`Fn`] which will return `true` if provided
    /// [`PeerStateMachine`]'s [`PeerId`] will be equal to the provided into
    /// [`peer_id_eq`] [`PeerId`].
    fn peer_id_eq(peer_id: u32) -> impl Fn(&PeerStateMachine) -> bool {
        move |peer| peer.id() == PeerId(peer_id)
    }

    /// Checks that newly created [`Peer`] will be created in the
    /// [`RtcStatsHandler`] and [`PeerTrafficWatcher`].
    #[actix_rt::test]
    async fn peer_is_registered_in_metrics_service() {
        let mut mock = MockPeerTrafficWatcher::new();
        mock.expect_register_room()
            .returning(|_, _| Box::pin(future::ok(())));
        mock.expect_unregister_room().returning(|_| {});
        let (register_peer_tx, mut register_peer_rx) = mpsc::unbounded();
        let register_peer_done =
            timeout(Duration::from_secs(1), register_peer_rx.next());
        mock.expect_register_peer().returning(move |_, _, _| {
            register_peer_tx.unbounded_send(()).unwrap();
            Box::pin(future::ok(()))
        });
        mock.expect_traffic_flows().returning(|_, _, _| {});
        mock.expect_traffic_stopped().returning(|_, _, _| {});

        let negotiation_sub = NegotiationSubMock::new();
        let negotiations = negotiation_sub.on_negotiation_needed();

        let mut metrics_service = MockRtcStatsHandler::new();
        metrics_service
            .expect_register_peer()
            .withf(peer_id_eq(0))
            .times(1)
            .return_const(());
        metrics_service
            .expect_register_peer()
            .withf(peer_id_eq(1))
            .times(1)
            .return_const(());
        metrics_service
            .expect_update_peer()
            .withf(peer_id_eq(0))
            .times(1)
            .return_const(());
        metrics_service
            .expect_update_peer()
            .withf(peer_id_eq(1))
            .times(1)
            .return_const(());

        let peers_service = PeersService::with_metrics_service(
            "test".into(),
            new_turn_auth_service_mock(),
            Arc::new(mock),
            Rc::new(negotiation_sub),
            Box::new(metrics_service),
        );

        let publisher = Member::new(
            "publisher".into(),
            Credential::Plain("test".into()),
            "test".into(),
            Duration::from_secs(10),
            Duration::from_secs(10),
            Duration::from_secs(5),
        );
        let receiver = Member::new(
            "receiver".into(),
            Credential::Plain("test".into()),
            "test".into(),
            Duration::from_secs(10),
            Duration::from_secs(10),
            Duration::from_secs(5),
        );
        let publish = WebRtcPublishEndpoint::new(
            "publish".to_string().into(),
            P2pMode::Always,
            publisher.downgrade(),
            false,
            AudioSettings::default(),
            VideoSettings::default(),
        );
        let play = WebRtcPlayEndpoint::new(
            "play-publisher".to_string().into(),
            SrcUri::try_from("local://test/publisher/publish".to_string())
                .unwrap(),
            publish.downgrade(),
            receiver.downgrade(),
            false,
        );

        let (src_peer_id, sink_peer_id) = peers_service
            .clone()
            .connect_endpoints(publish, play)
            .await
            .unwrap()
            .unwrap();

        peers_service.commit_scheduled_changes(src_peer_id).unwrap();
        peers_service.update_peer_tracks(src_peer_id).unwrap();
        peers_service.update_peer_tracks(sink_peer_id).unwrap();

        register_peer_done.await.unwrap().unwrap();

        let negotiate_peer_ids: HashSet<_> =
            negotiations.take(2).collect().await;
        assert!(negotiate_peer_ids.contains(&PeerId(0)));
        assert!(negotiate_peer_ids.contains(&PeerId(1)));
    }

    /// Check that when new `Endpoint`s added to the [`PeerService`], tracks
    /// count will be updated in the [`RtcStatsHandler`].
    #[actix_rt::test]
    async fn adding_new_endpoint_updates_peer_metrics() {
        let mut mock = MockPeerTrafficWatcher::new();
        mock.expect_register_room()
            .returning(|_, _| Box::pin(future::ok(())));
        mock.expect_unregister_room().returning(|_| {});
        let (register_peer_tx, register_peer_rx) = mpsc::unbounded();
        let register_peer_done = timeout(
            Duration::from_secs(1),
            register_peer_rx.take(4).collect::<Vec<_>>(),
        );
        mock.expect_register_peer().returning(move |_, _, _| {
            register_peer_tx.unbounded_send(()).unwrap();
            Box::pin(future::ok(()))
        });
        mock.expect_traffic_flows().returning(|_, _, _| {});
        mock.expect_traffic_stopped().returning(|_, _, _| {});

        let negotiation_sub = NegotiationSubMock::new();
        let negotiations = negotiation_sub.on_negotiation_needed();

        let mut metrics_service = MockRtcStatsHandler::new();
        metrics_service
            .expect_register_peer()
            .withf(peer_id_eq(0))
            .times(1)
            .return_const(());
        metrics_service
            .expect_register_peer()
            .withf(peer_id_eq(1))
            .times(1)
            .return_const(());
        metrics_service
            .expect_update_peer()
            .withf(peer_id_eq(0))
            .times(2)
            .return_const(());
        metrics_service
            .expect_update_peer()
            .withf(peer_id_eq(1))
            .times(2)
            .return_const(());

        let peers_service = PeersService::with_metrics_service(
            "test".into(),
            new_turn_auth_service_mock(),
            Arc::new(mock),
            Rc::new(negotiation_sub),
            Box::new(metrics_service),
        );

        let publisher = Member::new(
            "publisher".into(),
            Credential::Plain("test".into()),
            "test".into(),
            Duration::from_secs(10),
            Duration::from_secs(10),
            Duration::from_secs(5),
        );
        let receiver = Member::new(
            "receiver".into(),
            Credential::Plain("test".into()),
            "test".into(),
            Duration::from_secs(10),
            Duration::from_secs(10),
            Duration::from_secs(5),
        );
        let publish = WebRtcPublishEndpoint::new(
            "publish".to_string().into(),
            P2pMode::Always,
            publisher.downgrade(),
            false,
            AudioSettings::default(),
            VideoSettings::default(),
        );
        let play = WebRtcPlayEndpoint::new(
            "play-publisher".to_string().into(),
            SrcUri::try_from("local://test/publisher/publish".to_string())
                .unwrap(),
            publish.downgrade(),
            receiver.downgrade(),
            false,
        );

        let (src_peer_id, sink_peer_id) = peers_service
            .clone()
            .connect_endpoints(publish, play)
            .await
            .unwrap()
            .unwrap();

        peers_service.commit_scheduled_changes(src_peer_id).unwrap();
        peers_service.update_peer_tracks(src_peer_id).unwrap();
        peers_service.update_peer_tracks(sink_peer_id).unwrap();

        let publish = WebRtcPublishEndpoint::new(
            "publish".to_string().into(),
            P2pMode::Always,
            receiver.downgrade(),
            false,
            AudioSettings::default(),
            VideoSettings::default(),
        );
        let play = WebRtcPlayEndpoint::new(
            "play-publisher".to_string().into(),
            SrcUri::try_from("local://test/publisher/publish".to_string())
                .unwrap(),
            publish.downgrade(),
            publisher.downgrade(),
            false,
        );

        let (src_peer_id, sink_peer_id) = peers_service
            .clone()
            .connect_endpoints(publish, play)
            .await
            .unwrap()
            .unwrap();

        peers_service.commit_scheduled_changes(src_peer_id).unwrap();
        peers_service.update_peer_tracks(src_peer_id).unwrap();
        peers_service.update_peer_tracks(sink_peer_id).unwrap();

        register_peer_done.await.unwrap();

        let negotiate_peer_ids: HashSet<_> =
            negotiations.take(4).collect().await;
        assert!(negotiate_peer_ids.contains(&PeerId(0)));
        assert!(negotiate_peer_ids.contains(&PeerId(1)));
    }
}
