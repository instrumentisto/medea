//! Repository that stores [`Room`]s [`Peer`]s.

mod media_traffic_state;
mod metrics;
mod traffic_watcher;

use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    marker::PhantomData,
    sync::Arc,
    time::Duration,
};

use actix::{
    fut,
    fut::{wrap_future, Either},
    Actor, ActorFuture, WrapFuture as _,
};
use derive_more::Display;
use futures::{future, future::LocalBoxFuture, FutureExt};
use medea_client_api_proto::{Incrementable, PeerId, TrackId};

use crate::{
    api::control::{MemberId, RoomId},
    conf,
    log::prelude::*,
    media::{IceUser, Peer, PeerError, PeerStateMachine, Stable, WaitLocalSdp},
    signalling::{
        elements::endpoints::{
            webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            Endpoint,
        },
        room::RoomError,
        Room,
    },
    turn::{TurnAuthService, UnreachablePolicy},
};

use self::metrics::PeersMetricsService;

pub use self::{
    metrics::{PeersMetricsEvent, PeersMetricsEventHandler},
    traffic_watcher::{
        build_peers_traffic_watcher, FlowMetricSource,
        PeerConnectionStateEventsHandler, PeerTrafficWatcher,
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Debug, Clone)]
struct PeerRepository(Rc<RefCell<HashMap<PeerId, PeerStateMachine>>>);

impl PeerRepository {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(HashMap::new())))
    }

    pub fn map_peer_by_id<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        Ok((f(self
            .0
            .borrow()
            .get(&peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))?)))
    }

    pub fn map_peer_by_id_mut<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&mut PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        Ok((f(self
            .0
            .borrow_mut()
            .get_mut(&peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))?)))
    }

    pub fn remove(&self, peer_id: PeerId) -> Option<PeerStateMachine> {
        self.0.borrow_mut().remove(&peer_id)
    }

    pub fn take(&self, peer_id: PeerId) -> Result<PeerStateMachine, RoomError> {
        self.remove(peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))
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
        match self.take(peer_id)?.try_into() {
            Ok(peer) => Ok(peer),
            Err(err) => {
                let (err, peer) = err.into();
                self.add_peer(peer);
                Err(RoomError::from(err))
            }
        }
    }

    /// Store [`Peer`] in [`Room`].
    ///
    /// [`Room`]: crate::signalling::Room
    pub fn add_peer<S: Into<PeerStateMachine>>(&self, peer: S) {
        let peer = peer.into();
        self.0.borrow_mut().insert(peer.id(), peer);
    }

    /// Lookups [`Peer`] of [`Member`] with ID `member_id` which
    /// connected with `partner_member_id`.
    ///
    /// Returns `Some(peer_id, partner_peer_id)` if [`Peer`] has been found,
    /// otherwise returns `None`.
    pub fn get_peers_between_members(
        &self,
        member_id: &MemberId,
        partner_member_id: &MemberId,
    ) -> Option<(PeerId, PeerId)> {
        for peer in self.0.borrow().values() {
            if &peer.member_id() == member_id
                && &peer.partner_member_id() == partner_member_id
            {
                return Some((peer.id(), peer.partner_peer_id()));
            }
        }

        None
    }

    /// Removes all [`Peer`]s related to given [`Member`].
    /// Note, that this function will also remove all partners [`Peer`]s.
    ///
    /// Returns [`HashMap`] with all removed [`Peer`]s:
    /// key - [`Peer`]'s owner [`MemberId`],
    /// value - removed [`Peer`]'s [`PeerId`].
    // TODO: remove in #91.
    pub fn remove_peers_related_to_member(
        &mut self,
        member_id: &MemberId,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        let mut peers_to_remove: HashMap<MemberId, Vec<PeerId>> =
            HashMap::new();

        self.0
            .borrow()
            .values()
            .filter(|p| &p.member_id() == member_id)
            .for_each(|peer| {
                self.0
                    .borrow()
                    .values()
                    .filter(|p| p.member_id() == peer.partner_member_id())
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
            .for_each(|id| {
                self.0.borrow_mut().remove(id);
            });

        peers_to_remove
    }
}

#[derive(Debug)]
pub struct PeersService<A> {
    /// [`RoomId`] of the [`Room`] which owns this [`PeerRepository`].
    room_id: RoomId,

    /// [`TurnAuthService`] that [`IceUser`]s for the [`PeerConnection`]s from
    /// this [`PeerRepository`] will be created with.
    turn_service: Arc<dyn TurnAuthService>,

    /// [`Peer`]s of [`Member`]s in this [`Room`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    /// [`Room`]: crate::signalling::Room
    peers: PeerRepository,

    /// Count of [`Peer`]s in this [`Room`].
    ///
    /// [`Room`]: crate::signalling::room::Room
    peers_count: Counter<PeerId>,

    /// Count of [`MediaTrack`]s in this [`Room`].
    ///
    /// [`MediaTrack`]: crate::media::track::MediaTrack
    /// [`Room`]: crate::signalling::room::Room
    tracks_count: Counter<TrackId>,

    /// [`PeerTrafficWatcher`] which analyzes [`Peer`]s traffic metrics.
    peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,

    /// Service which responsible for this [`Room`]'s [`RtcStat`]s processing.
    peer_metrics_service: PeersMetricsService,

    /// Duration, after which [`Peer`]s stats will be considered as stale.
    /// Passed to [`PeersMetricsService`] when registering new [`Peer`]s.
    peer_stats_ttl: Duration,

    /// Type of the [`Actor`] in whose context will be spawned [`ActFuture`]s.
    _owner_actor: PhantomData<A>,
}

/// Simple ID counter.
#[derive(Default, Debug, Clone, Display)]
pub struct Counter<T: Copy> {
    count: Rc<Cell<T>>,
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
#[derive(Debug, Clone, Copy)]
enum GetOrCreatePeersResult {
    /// Requested [`Peer`] pair was created.
    Created(PeerId, PeerId),

    /// Requested [`Peer`] pair already existed.
    AlreadyExisted(PeerId, PeerId),
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectEndpointsResult {
    Created(PeerId, PeerId),

    Updated(PeerId, PeerId),

    NoOp(PeerId, PeerId),
}

/// [`Actor`] in whose context will be spawned [`ActFuture`]s returned from the
/// [`PeerService`].
pub trait PeerServiceOwner: Actor {
    /// Returns [`RoomId`] which owns [`PeerService`].
    fn id(&self) -> &RoomId;

    /// Returns reference to the [`PeersService`].
    fn peers(&self) -> &PeersService<Self>;

    /// Returns mutable reference to the [`PeersService`].
    fn peers_mut(&mut self) -> &mut PeersService<Self>;
}

impl PeerServiceOwner for Room {
    /// Returns reference to the [`Room::peers`].
    fn peers(&self) -> &PeersService<Self> {
        &self.peers
    }

    /// Returns mutable reference to the [`Room::peers`].
    fn peers_mut(&mut self) -> &mut PeersService<Self> {
        &mut self.peers
    }

    /// Returns reference to the [`Room::id`].
    fn id(&self) -> &RoomId {
        self.id()
    }
}


impl<A: Actor + PeerServiceOwner> PeersService<A> {
    /// Returns new [`PeerRepository`] for a [`Room`] with the provided
    /// [`RoomId`].
    pub fn new(
        room_id: RoomId,
        turn_service: Arc<dyn TurnAuthService>,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
        media_conf: &conf::Media,
    ) -> Self {
        Self {
            room_id: room_id.clone(),
            turn_service,
            peers: PeerRepository::new(),
            peers_count: Counter::default(),
            tracks_count: Counter::default(),
            peers_traffic_watcher: peers_traffic_watcher.clone(),
            peer_metrics_service: PeersMetricsService::new(
                room_id,
                peers_traffic_watcher,
            ),
            peer_stats_ttl: media_conf.max_lag,
            _owner_actor: PhantomData::default(),
        }
    }

    /// Store [`Peer`] in [`Room`].
    ///
    /// [`Room`]: crate::signalling::Room
    pub fn add_peer<S: Into<PeerStateMachine>>(&self, peer: S) {
        self.peers.add_peer(peer)
    }

    pub fn map_peer_by_id<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        self.peers.map_peer_by_id(peer_id, f)
    }

    pub fn map_peer_by_id_mut<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&mut PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        self.peers.map_peer_by_id_mut(peer_id, f)
    }

    /// Creates interconnected [`Peer`]s for provided endpoints and saves them
    /// in [`PeerService`].
    ///
    /// Returns [`PeerId`]s of the created [`Peer`]s.
    fn create_peers(
        &mut self,
        src: &WebRtcPublishEndpoint,
        sink: &WebRtcPlayEndpoint,
    ) -> (PeerId, PeerId) {
        let src_member_id = src.owner().id();
        let sink_member_id = sink.owner().id();

        debug!(
            "Created peer between {} and {}.",
            src_member_id, sink_member_id
        );
        let src_peer_id = self.peers_count.next_id();
        let sink_peer_id = self.peers_count.next_id();

        let mut src_peer = Peer::new(
            src_peer_id,
            src_member_id.clone(),
            sink_peer_id,
            sink_member_id.clone(),
            src.is_force_relayed(),
        );
        src_peer.add_endpoint(&src.clone().into());

        let mut sink_peer = Peer::new(
            sink_peer_id,
            sink_member_id,
            src_peer_id,
            src_member_id,
            sink.is_force_relayed(),
        );
        sink_peer.add_endpoint(&sink.clone().into());

        src_peer.add_publisher(&mut sink_peer, self.get_tracks_counter());

        let src_peer = PeerStateMachine::from(src_peer);
        let sink_peer = PeerStateMachine::from(sink_peer);

        self.peer_metrics_service
            .register_peer(&src_peer, self.peer_stats_ttl);
        self.peer_metrics_service
            .register_peer(&sink_peer, self.peer_stats_ttl);

        self.add_peer(src_peer);
        self.add_peer(sink_peer);

        (src_peer_id, sink_peer_id)
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
        &mut self,
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
    pub fn remove_peers<'a, Peers: IntoIterator<Item = &'a PeerId>>(
        &mut self,
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
                        .entry(partner_member_id)
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
            .unregister_peers(&peers_to_unregister);
        self.peers_traffic_watcher
            .unregister_peers(self.room_id.clone(), peers_to_unregister);

        removed_peers
    }

    /// Returns already created [`Peer`] pair's [`PeerId`]s as
    /// [`CreatedOrGottenPeer::Gotten`] variant.
    ///
    /// Returns newly created [`Peer`] pair's [`PeerId`]s as
    /// [`CreatedOrGottenPeer::Created`] variant.
    fn get_or_create_peers(
        &mut self,
        src: WebRtcPublishEndpoint,
        sink: WebRtcPlayEndpoint,
    ) -> LocalBoxFuture<'static, Result<GetOrCreatePeersResult, RoomError>>
    {
        match self
            .get_peers_between_members(&src.owner().id(), &sink.owner().id())
        {
            Some((first_peer_id, second_peer_id)) => {
                future::ok(GetOrCreatePeersResult::AlreadyExisted(
                    first_peer_id,
                    second_peer_id,
                ))
                .boxed_local()
            }
            None => {
                let (src_peer_id, sink_peer_id) =
                    self.create_peers(&src, &sink);

                let src_peer_post_construct =
                    self.peer_post_construct(src_peer_id, &src.into());
                let sink_peer_post_construct =
                    self.peer_post_construct(sink_peer_id, &sink.into());
                async move {
                    src_peer_post_construct.await?;
                    sink_peer_post_construct.await?;

                    Ok(GetOrCreatePeersResult::Created(
                        src_peer_id,
                        sink_peer_id,
                    ))
                }
                .boxed_local()
            }
        }
    }

    /// Creates and sets [`IceUser`], registers [`Peer`] in
    /// [`PeerTrafficWatcher`].
    fn peer_post_construct(
        &self,
        peer_id: PeerId,
        endpoint: &Endpoint,
    ) -> LocalBoxFuture<'static, Result<(), RoomError>> {
        let room_id = self.room_id.clone();
        let room_id_clone = self.room_id.clone();
        let turn_service = self.turn_service.clone();
        let has_traffic_callback = endpoint.has_traffic_callback();
        let is_force_relayed = endpoint.is_force_relayed();
        let peers = self.peers.clone();
        let traffic_watcher = self.peers_traffic_watcher.clone();

        async move {
            let ice_user = turn_service
                .create(room_id, peer_id, UnreachablePolicy::ReturnErr)
                .await?;

            let _ = peers
                .map_peer_by_id_mut(peer_id, move |p| p.set_ice_user(ice_user));

            if has_traffic_callback {
                traffic_watcher
                    .register_peer(room_id_clone, peer_id, is_force_relayed)
                    .await
                    .map_err(RoomError::PeerTrafficWatcherMailbox)
            } else {
                Ok(())
            }
        }
        .boxed_local()
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
        src: WebRtcPublishEndpoint,
        sink: WebRtcPlayEndpoint,
    ) -> LocalBoxFuture<'static, Result<ConnectEndpointsResult, RoomError>>
    {
        use ConnectEndpointsResult::{Created, NoOp, Updated};

        debug!(
            "Connecting endpoints of Member [id = {}] with Member [id = {}]",
            src.owner().id(),
            sink.owner().id(),
        );
        let get_or_create_peers =
            self.get_or_create_peers(src.clone(), sink.clone());
        let peers = self.peers.clone();
        let track_count = self.tracks_count.clone();
        let peers_traffic_watcher = self.peers_traffic_watcher.clone();
        let room_id = self.room_id.clone();
        async move {
            match get_or_create_peers.await? {
                GetOrCreatePeersResult::Created(src_peer_id, sink_peer_id) => {
                    Ok(Created(src_peer_id, sink_peer_id))
                }
                GetOrCreatePeersResult::AlreadyExisted(
                    src_peer_id,
                    sink_peer_id,
                ) => {
                    if sink.peer_id().is_some()
                        || src.peer_ids().contains(&src_peer_id)
                    {
                        // already connected, so no-op
                        Ok(NoOp(src_peer_id, sink_peer_id))
                    } else {
                        let mut futs = Vec::new();
                        // TODO: here we assume that peers are stable,
                        //       which might not be the case, e.g. Control
                        //       Service creates multiple endpoints in quick
                        //       succession.
                        let mut src_peer: Peer<Stable> =
                            peers.take_inner_peer(src_peer_id).unwrap();
                        let mut sink_peer: Peer<Stable> =
                            peers.take_inner_peer(sink_peer_id).unwrap();

                        src_peer.add_publisher(&mut sink_peer, &track_count);

                        if src.has_traffic_callback() {
                            futs.push(peers_traffic_watcher.register_peer(
                                room_id.clone(),
                                src_peer_id,
                                src.is_force_relayed(),
                            ));
                        }
                        if sink.has_traffic_callback() {
                            futs.push(peers_traffic_watcher.register_peer(
                                room_id.clone(),
                                sink_peer_id,
                                sink.is_force_relayed(),
                            ));
                        }

                        sink_peer.add_endpoint(&sink.into());
                        src_peer.add_endpoint(&src.into());

                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // TODO
                        // this.peer_metrics_service
                        //     .update_peer_tracks(&src_peer);
                        // this.peer_metrics_service
                        //     .update_peer_tracks(&sink_peer);

                        peers.add_peer(src_peer);
                        peers.add_peer(sink_peer);

                        future::try_join_all(futs)
                            .await
                            .map_err(RoomError::PeerTrafficWatcherMailbox)?;

                        Ok(Updated(src_peer_id, sink_peer_id))
                    }
                }
            }
        }
        .boxed_local()
    }

    /// Removes all [`Peer`]s related to given [`Member`].
    /// Note, that this function will also remove all partners [`Peer`]s.
    ///
    /// Returns [`HashMap`] with all removed [`Peer`]s:
    /// key - [`Peer`]'s owner [`MemberId`],
    /// value - removed [`Peer`]'s [`PeerId`].
    // TODO: remove in #91.
    pub fn remove_peers_related_to_member(
        &mut self,
        member_id: &MemberId,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        self.peers.remove_peers_related_to_member(member_id)
    }

    /// Adds new [`WebRtcPlayEndpoint`] to the [`Peer`] with a provided
    /// [`PeerId`].
    pub fn add_sink(&mut self, peer_id: PeerId, sink: WebRtcPlayEndpoint) {
        let mut peer: Peer<Stable> = self.take_inner_peer(peer_id).unwrap();
        let mut partner_peer: Peer<Stable> =
            self.take_inner_peer(peer.partner_peer_id()).unwrap();

        peer.add_publisher(&mut partner_peer, &mut self.tracks_count);
        peer.add_endpoint(&Endpoint::from(sink));

        self.peers.add_peer(peer);
        self.peers.add_peer(partner_peer);
    }
}

#[cfg(test)]
mod tests {
    use actix::{Actor, Handler, Message, WrapFuture};
    use futures::{channel::mpsc, future, StreamExt as _};
    use tokio::time::timeout;

    use crate::{
        api::control::{
            endpoints::webrtc_publish_endpoint::P2pMode, refs::SrcUri,
        },
        signalling::{
            elements::Member, peers::traffic_watcher::MockPeerTrafficWatcher,
        },
        turn::service::test::new_turn_auth_service_mock,
    };

    use super::*;

    /// Mock for the [`PeersServiceOwner`] trait.
    ///
    /// In context of this [`Actor`] will be ran all [`ActFuture`]s received
    /// from the [`PeerService`].
    struct PeersServiceOwnerMock {
        /// Actual [`PeersService`].
        peers_service: PeersService<PeersServiceOwnerMock>,

        /// All [`Member`]s which should be in the [`Room`].
        ///
        /// This is necessary so that [`Drop`] is not called on the created
        /// [`Member`]s.
        members: Vec<Member>,
    }

    impl PeersServiceOwnerMock {
        /// Returns empty [`PeersServiceOwnerMock`] with provided
        /// [`PeerService`].
        pub fn new(peers: PeersService<PeersServiceOwnerMock>) -> Self {
            Self {
                peers_service: peers,
                members: Vec::new(),
            }
        }
    }

    impl Actor for PeersServiceOwnerMock {
        type Context = actix::Context<Self>;
    }

    impl PeerServiceOwner for PeersServiceOwnerMock {
        /// Returns reference to the [`RoomId`] from the [`PeerService`].
        fn id(&self) -> &RoomId {
            &self.peers_service.room_id
        }

        /// Returns reference to the [`PeersServiceOwnerMock::peers_service`].
        fn peers(&self) -> &PeersService<Self> {
            &self.peers_service
        }

        /// Returns mutable reference to the
        /// [`PeersServiceOwnerMcok::peers_service`].
        fn peers_mut(&mut self) -> &mut PeersService<Self> {
            &mut self.peers_service
        }
    }

    /// Checks that newly created [`Peer`] will be created in the
    /// [`PeerMetricsService`] and [`PeerTrafficWatcher`].
    #[actix_rt::test]
    async fn peer_is_registered_in_metrics_service() {
        #[derive(Message)]
        #[rtype(result = "Result<(), ()>")]
        struct RunTest;

        impl Handler<RunTest> for PeersServiceOwnerMock {
            type Result = ActFuture<PeersServiceOwnerMock, Result<(), ()>>;

            fn handle(
                &mut self,
                _: RunTest,
                _: &mut Self::Context,
            ) -> Self::Result {
                let publisher = Member::new(
                    "publisher".into(),
                    "test".to_string(),
                    "test".into(),
                    Duration::from_secs(10),
                    Duration::from_secs(10),
                    Duration::from_secs(5),
                );
                let receiver = Member::new(
                    "receiver".into(),
                    "test".to_string(),
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
                );
                let play = WebRtcPlayEndpoint::new(
                    "play-publisher".to_string().into(),
                    SrcUri::try_from(
                        "local://test/publisher/publish".to_string(),
                    )
                    .unwrap(),
                    publish.downgrade(),
                    receiver.downgrade(),
                    false,
                );

                self.members.push(publisher);
                self.members.push(receiver);

                let fut =
                    PeersService::<PeersServiceOwnerMock>::connect_endpoints(
                        publish, play,
                    );

                Box::new(fut.then(|res, this, _| {
                    res.unwrap();

                    assert!(this
                        .peers_service
                        .peer_metrics_service
                        .is_peer_registered(PeerId(0)));
                    assert!(this
                        .peers_service
                        .peer_metrics_service
                        .is_peer_registered(PeerId(1)));

                    async { Ok(()) }.into_actor(this)
                }))
            }
        }

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

        let peers: PeersService<PeersServiceOwnerMock> = PeersService::new(
            "test".into(),
            new_turn_auth_service_mock(),
            Arc::new(mock),
            &conf::Media::default(),
        );

        let runner = PeersServiceOwnerMock::new(peers).start();
        runner.send(RunTest).await.unwrap().unwrap();
        register_peer_done.await.unwrap().unwrap();
    }

    /// Check that when new `Endpoint`s added to the [`PeerService`], tracks
    /// count will be updated in the [`PeerMetricsService`].
    #[actix_rt::test]
    async fn adding_new_endpoint_updates_peer_metrics() {
        #[derive(Message)]
        #[rtype(result = "Result<(), ()>")]
        struct RunTest;

        impl Handler<RunTest> for PeersServiceOwnerMock {
            type Result = ActFuture<PeersServiceOwnerMock, Result<(), ()>>;

            fn handle(
                &mut self,
                _: RunTest,
                _: &mut Self::Context,
            ) -> Self::Result {
                let publisher = Member::new(
                    "publisher".into(),
                    "test".to_string(),
                    "test".into(),
                    Duration::from_secs(10),
                    Duration::from_secs(10),
                    Duration::from_secs(5),
                );
                let receiver = Member::new(
                    "receiver".into(),
                    "test".to_string(),
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
                );
                let play = WebRtcPlayEndpoint::new(
                    "play-publisher".to_string().into(),
                    SrcUri::try_from(
                        "local://test/publisher/publish".to_string(),
                    )
                    .unwrap(),
                    publish.downgrade(),
                    receiver.downgrade(),
                    false,
                );

                self.members.push(publisher.clone());
                self.members.push(receiver.clone());

                let fut =
                    PeersService::<PeersServiceOwnerMock>::connect_endpoints(
                        publish, play,
                    );

                Box::new(fut.then(move |res, this, _| {
                    res.unwrap();

                    let first_peer_tracks_count = this
                        .peers_service
                        .peer_metrics_service
                        .peer_tracks_count(PeerId(0));
                    assert_eq!(first_peer_tracks_count, 2);
                    let second_peer_tracks_count = this
                        .peers_service
                        .peer_metrics_service
                        .peer_tracks_count(PeerId(1));
                    assert_eq!(second_peer_tracks_count, 2);

                    let publish = WebRtcPublishEndpoint::new(
                        "publish".to_string().into(),
                        P2pMode::Always,
                        receiver.downgrade(),
                        false,
                    );
                    let play = WebRtcPlayEndpoint::new(
                        "play-publisher".to_string().into(),
                        SrcUri::try_from(
                            "local://test/publisher/publish".to_string(),
                        )
                        .unwrap(),
                        publish.downgrade(),
                        publisher.downgrade(),
                        false,
                    );

                    PeersService::<PeersServiceOwnerMock>::connect_endpoints(
                        publish, play,
                    )
                    .then(|res, this, _| {
                        res.unwrap();
                        let first_peer_tracks_count = this
                            .peers_service
                            .peer_metrics_service
                            .peer_tracks_count(PeerId(0));
                        assert_eq!(first_peer_tracks_count, 4);
                        let second_peer_tracks_count = this
                            .peers_service
                            .peer_metrics_service
                            .peer_tracks_count(PeerId(1));
                        assert_eq!(second_peer_tracks_count, 4);

                        async { Ok(()) }.into_actor(this)
                    })
                }))
            }
        }

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

        let peers: PeersService<PeersServiceOwnerMock> = PeersService::new(
            "test".into(),
            new_turn_auth_service_mock(),
            Arc::new(mock),
            &conf::Media::default(),
        );

        let runner = PeersServiceOwnerMock::new(peers).start();
        runner.send(RunTest).await.unwrap().unwrap();
        register_peer_done.await.unwrap();
    }
}
