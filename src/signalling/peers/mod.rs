//! Repository that stores [`Room`]s [`Peer`]s.

mod media_traffic_state;
mod metrics;
mod traffic_watcher;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    convert::{TryFrom, TryInto},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use derive_more::Display;
use futures::future;
use medea_client_api_proto::{Incrementable, PeerId, TrackId};

use crate::{
    api::control::{MemberId, RoomId},
    conf,
    log::prelude::*,
    media::{peer::NegotiationSubscriber, Peer, PeerError, PeerStateMachine},
    signalling::{
        elements::endpoints::{
            webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            Endpoint,
        },
        room::RoomError,
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

#[derive(Debug)]
pub struct PeersService {
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
    peer_metrics_service: RefCell<PeersMetricsService>,

    /// Duration, after which [`Peer`]s stats will be considered as stale.
    /// Passed to [`PeersMetricsService`] when registering new [`Peer`]s.
    peer_stats_ttl: Duration,

    /// Subscriber to the events which indicates that negotiation process
    /// should be started for a some [`Peer`].
    negotiation_sub: Rc<dyn NegotiationSubscriber>,
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

/// Result of the [`PeersService::connect_endpoints`] function.
#[derive(Clone, Copy, Debug)]
pub enum ConnectEndpointsResult {
    /// New [`Peer`] pair was created.
    Created(PeerId, PeerId),

    /// [`Peer`] pair was updated.
    Updated(PeerId, PeerId),
}

impl PeersService {
    /// Returns new [`PeerRepository`] for a [`Room`] with the provided
    /// [`RoomId`].
    pub fn new(
        room_id: RoomId,
        turn_service: Arc<dyn TurnAuthService>,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
        media_conf: &conf::Media,
        negotiation_sub: Rc<dyn NegotiationSubscriber>,
    ) -> Rc<Self> {
        Rc::new(Self {
            room_id: room_id.clone(),
            turn_service,
            peers: PeerRepository::new(),
            peers_count: Counter::default(),
            tracks_count: Counter::default(),
            peers_traffic_watcher: peers_traffic_watcher.clone(),
            peer_metrics_service: RefCell::new(PeersMetricsService::new(
                room_id,
                peers_traffic_watcher,
            )),
            peer_stats_ttl: media_conf.max_lag,
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
    /// in [`PeerService`].
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
            .register_peer(&src_peer, self.peer_stats_ttl);
        self.peer_metrics_service
            .borrow_mut()
            .register_peer(&sink_peer, self.peer_stats_ttl);

        self.add_peer(src_peer);
        self.add_peer(sink_peer);

        (src_peer_id, sink_peer_id)
    }

    /// Lookups [`Peer`] of [`Member`] with ID `member_id` which
    /// connected with `partner_member_id`.
    ///
    /// Returns `Some(peer_id, partner_peer_id)` if [`Peer`] has been found,
    /// otherwise returns `None`.
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
            .borrow_mut()
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

    /// Calls [`PeerStateMachine::run_scheduled_jobs`] on the
    /// [`PeerStateMachine`] with a provided [`PeerId`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub fn run_scheduled_jobs(&self, peer_id: PeerId) -> Result<(), RoomError> {
        self.peers.map_peer_by_id_mut(peer_id, |peer| {
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
    async fn peer_post_construct(
        &self,
        peer_id: PeerId,
        endpoint: &Endpoint,
    ) -> Result<(), RoomError> {
        let ice_user = self
            .turn_service
            .create(self.room_id.clone(), peer_id, UnreachablePolicy::ReturnErr)
            .await?;

        self.peers
            .map_peer_by_id_mut(peer_id, move |p| p.set_ice_user(ice_user))?;

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
    pub fn update_peer_tracks(&self, peer_id: PeerId) -> Result<(), RoomError> {
        self.peers.map_peer_by_id(peer_id, |peer| {
            self.peer_metrics_service
                .borrow_mut()
                .update_peer_tracks(peer);
        })?;

        Ok(())
    }

    /// Removes all [`Peer`]s related to given [`Member`].
    /// Note, that this function will also remove all partners [`Peer`]s.
    ///
    /// Returns [`HashMap`] with all removed [`Peer`]s:
    /// key - [`Peer`]'s owner [`MemberId`],
    /// value - removed [`Peer`]'s [`PeerId`].
    // TODO: remove in #91.
    #[inline]
    pub fn remove_peers_related_to_member(
        &self,
        member_id: &MemberId,
    ) -> HashMap<MemberId, Vec<PeerId>> {
        self.peers.remove_peers_related_to_member(member_id)
    }

    /// Updates [`PeerTracks`] of the [`Peer`] with provided [`PeerId`] in the
    /// [`PeerMetricsService`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub fn sync_peer_spec(&self, peer_id: PeerId) -> Result<(), RoomError> {
        self.peers.map_peer_by_id(peer_id, |peer| {
            self.peer_metrics_service
                .borrow_mut()
                .update_peer_tracks(&peer);
        })?;
        Ok(())
    }
}

/// Repository which stores all [`PeerStateMachine`]s of the [`PeersService`].
#[derive(Debug)]
struct PeerRepository(RefCell<HashMap<PeerId, PeerStateMachine>>);

impl PeerRepository {
    /// Returns empty [`PeerRepository`].
    pub fn new() -> Self {
        Self(RefCell::new(HashMap::new()))
    }

    /// Applies a function to the [`PeerStateMachine`] reference with provided
    /// [`PeerId`] (if any found).
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub fn map_peer_by_id<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        Ok(f(self
            .0
            .borrow()
            .get(&peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))?))
    }

    /// Applies a function to the mutable [`PeerStateMachine`] reference with
    /// provided [`PeerId`] (if any found).
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub fn map_peer_by_id_mut<T>(
        &self,
        peer_id: PeerId,
        f: impl FnOnce(&mut PeerStateMachine) -> T,
    ) -> Result<T, RoomError> {
        Ok(f(self
            .0
            .borrow_mut()
            .get_mut(&peer_id)
            .ok_or_else(|| RoomError::PeerNotFound(peer_id))?))
    }

    /// Removes [`PeerStateMachine`] with a provided [`PeerId`].
    ///
    /// Returns removed [`PeerStateMachine`] if it existed.
    pub fn remove(&self, peer_id: PeerId) -> Option<PeerStateMachine> {
        self.0.borrow_mut().remove(&peer_id)
    }

    /// Removes [`PeerStateMachine`] with a provided [`PeerId`] and returns
    /// removed [`PeerStateMachine`] if it existed.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
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
        &self,
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use futures::{channel::mpsc, future, Stream, StreamExt as _};
    use tokio::time::timeout;

    use crate::{
        api::control::{
            endpoints::webrtc_publish_endpoint::{
                AudioSettings, P2pMode, VideoSettings,
            },
            refs::SrcUri,
        },
        signalling::{
            elements::Member, peers::traffic_watcher::MockPeerTrafficWatcher,
        },
        turn::service::test::new_turn_auth_service_mock,
    };

    use super::*;

    /// Mock for the [`NegotiationSubscriber`] trait.
    ///
    /// You can subscribe to the [`Stream`] into which will be sent all
    /// [`PeerId`]s of [`Peer`] which are should be renegotiated.
    #[derive(Debug, Clone)]
    struct NegotiationSubMock(Rc<RefCell<Vec<mpsc::UnboundedSender<PeerId>>>>);

    impl NegotiationSubMock {
        /// Returns new empty [`NegotiationSubMock`].
        pub fn new() -> Self {
            Self(Rc::new(RefCell::new(Vec::new())))
        }

        /// Returns [`Stream`] into which will be sent all [`PeerId`]s of
        /// [`Peer`] which are should be renegotiated.
        pub fn subscribe(&self) -> impl Stream<Item = PeerId> {
            let (tx, rx) = mpsc::unbounded();

            self.0.borrow_mut().push(tx);

            rx
        }
    }

    impl NegotiationSubscriber for NegotiationSubMock {
        /// Sends [`PeerId`] to the [`NegotiationSubMock::subscribe`]
        /// [`Stream`].
        fn negotiation_needed(&self, peer_id: PeerId) {
            self.0.borrow().iter().for_each(|sender| {
                let _ = sender.unbounded_send(peer_id);
            });
        }
    }

    /// Checks that newly created [`Peer`] will be created in the
    /// [`PeerMetricsService`] and [`PeerTrafficWatcher`].
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
        let negotiations = negotiation_sub.subscribe();

        let peers_service = PeersService::new(
            "test".into(),
            new_turn_auth_service_mock(),
            Arc::new(mock),
            &conf::Media::default(),
            Rc::new(negotiation_sub),
        );

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

        peers_service.run_scheduled_jobs(src_peer_id).unwrap();
        peers_service.run_scheduled_jobs(sink_peer_id).unwrap();
        peers_service.update_peer_tracks(src_peer_id).unwrap();
        peers_service.update_peer_tracks(sink_peer_id).unwrap();

        register_peer_done.await.unwrap().unwrap();

        assert!(peers_service
            .peer_metrics_service
            .borrow()
            .is_peer_registered(PeerId(0)));
        assert!(peers_service
            .peer_metrics_service
            .borrow()
            .is_peer_registered(PeerId(1)));

        let negotiate_peer_ids: HashSet<_> =
            negotiations.take(2).collect().await;
        assert!(negotiate_peer_ids.contains(&PeerId(0)));
        assert!(negotiate_peer_ids.contains(&PeerId(1)));
    }

    /// Check that when new `Endpoint`s added to the [`PeerService`], tracks
    /// count will be updated in the [`PeerMetricsService`].
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
        let negotiations = negotiation_sub.subscribe();

        let peers_service = PeersService::new(
            "test".into(),
            new_turn_auth_service_mock(),
            Arc::new(mock),
            &conf::Media::default(),
            Rc::new(negotiation_sub),
        );

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

        peers_service.run_scheduled_jobs(src_peer_id).unwrap();
        peers_service.run_scheduled_jobs(sink_peer_id).unwrap();
        peers_service.update_peer_tracks(src_peer_id).unwrap();
        peers_service.update_peer_tracks(sink_peer_id).unwrap();

        let first_peer_tracks_count = peers_service
            .peer_metrics_service
            .borrow()
            .peer_tracks_count(PeerId(0));
        assert_eq!(first_peer_tracks_count, 2);
        let second_peer_tracks_count = peers_service
            .peer_metrics_service
            .borrow()
            .peer_tracks_count(PeerId(1));
        assert_eq!(second_peer_tracks_count, 2);

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

        peers_service.run_scheduled_jobs(src_peer_id).unwrap();
        peers_service.run_scheduled_jobs(sink_peer_id).unwrap();
        peers_service.update_peer_tracks(src_peer_id).unwrap();
        peers_service.update_peer_tracks(sink_peer_id).unwrap();

        let first_peer_tracks_count = peers_service
            .peer_metrics_service
            .borrow()
            .peer_tracks_count(PeerId(0));
        assert_eq!(first_peer_tracks_count, 4);
        let second_peer_tracks_count = peers_service
            .peer_metrics_service
            .borrow()
            .peer_tracks_count(PeerId(1));
        assert_eq!(second_peer_tracks_count, 4);

        register_peer_done.await.unwrap();

        let negotiate_peer_ids: HashSet<_> =
            negotiations.take(4).collect().await;
        assert!(negotiate_peer_ids.contains(&PeerId(0)));
        assert!(negotiate_peer_ids.contains(&PeerId(1)));
    }
}
