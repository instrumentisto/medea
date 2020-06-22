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
    media::{Peer, PeerError, PeerStateMachine, Stable},
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

#[derive(Debug)]
pub struct PeersServiceInner {
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
}

#[derive(Clone, Debug)]
pub struct PeersService(Rc<PeersServiceInner>);

/// Simple ID counter.
#[derive(Default, Debug, Clone, Display)]
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
#[derive(Debug, Clone, Copy)]
enum GetOrCreatePeersResult {
    /// Requested [`Peer`] pair was created.
    Created(PeerId, PeerId),

    /// Requested [`Peer`] pair already existed.
    AlreadyExisted(PeerId, PeerId),
}

/// Result of the [`PeersService::connect_endpoints`] function.
#[derive(Debug, Clone, Copy)]
pub enum ConnectEndpointsResult {
    /// New [`Peer`] pair was created.
    Created(PeerId, PeerId),

    /// [`Peer`] pair was updated.
    Updated(PeerId, PeerId),

    /// Nothing was done because endpoints already interconnected.
    NoOp(PeerId, PeerId),
}

impl PeersService {
    /// Returns new [`PeerRepository`] for a [`Room`] with the provided
    /// [`RoomId`].
    pub fn new(
        room_id: RoomId,
        turn_service: Arc<dyn TurnAuthService>,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
        media_conf: &conf::Media,
    ) -> Self {
        Self(Rc::new(PeersServiceInner {
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
        }))
    }

    /// Store [`Peer`] in [`Room`].
    ///
    /// [`Room`]: crate::signalling::Room
    #[inline]
    pub fn add_peer<S: Into<PeerStateMachine>>(&self, peer: S) {
        self.0.peers.add_peer(peer)
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
        self.0.peers.map_peer_by_id(peer_id, f)
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

        let src_peer_id = self.0.peers_count.next_id();
        let sink_peer_id = self.0.peers_count.next_id();

        debug!(
            "Created peers:[{}, {}] between {} and {}.",
            src_peer_id, sink_peer_id, src_member_id, sink_member_id,
        );

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

        src_peer.add_publisher(&src, &mut sink_peer, &self.0.tracks_count);

        let src_peer = PeerStateMachine::from(src_peer);
        let sink_peer = PeerStateMachine::from(sink_peer);

        self.0
            .peer_metrics_service
            .borrow_mut()
            .register_peer(&src_peer, self.0.peer_stats_ttl);
        self.0
            .peer_metrics_service
            .borrow_mut()
            .register_peer(&sink_peer, self.0.peer_stats_ttl);

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
        self.0
            .peers
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
        self.0.peers.take_inner_peer(peer_id)
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
            if let Some(peer) = self.0.peers.remove(*peer_id) {
                let partner_peer_id = peer.partner_peer_id();
                let partner_member_id = peer.partner_member_id();
                if let Some(partner_peer) = self.0.peers.remove(partner_peer_id)
                {
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
        self.0
            .peer_metrics_service
            .borrow_mut()
            .unregister_peers(&peers_to_unregister);
        self.0
            .peers_traffic_watcher
            .unregister_peers(self.0.room_id.clone(), peers_to_unregister);

        removed_peers
    }

    /// Returns already created [`Peer`] pair's [`PeerId`]s as
    /// [`CreatedOrGottenPeer::Gotten`] variant.
    ///
    /// Returns newly created [`Peer`] pair's [`PeerId`]s as
    /// [`CreatedOrGottenPeer::Created`] variant.
    async fn get_or_create_peers(
        self,
        src: WebRtcPublishEndpoint,
        sink: WebRtcPlayEndpoint,
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

            self.clone()
                .peer_post_construct(src_peer_id, &src.into())
                .await?;
            self.clone()
                .peer_post_construct(sink_peer_id, &sink.into())
                .await?;

            Ok(GetOrCreatePeersResult::Created(src_peer_id, sink_peer_id))
        }
    }

    /// Creates and sets [`IceUser`], registers [`Peer`] in
    /// [`PeerTrafficWatcher`].
    async fn peer_post_construct(
        self,
        peer_id: PeerId,
        endpoint: &Endpoint,
    ) -> Result<(), RoomError> {
        let has_traffic_callback = endpoint.has_traffic_callback();
        let is_force_relayed = endpoint.is_force_relayed();

        let ice_user = self
            .0
            .turn_service
            .create(
                self.0.room_id.clone(),
                peer_id,
                UnreachablePolicy::ReturnErr,
            )
            .await?;

        let _ = self
            .0
            .peers
            .map_peer_by_id_mut(peer_id, move |p| p.set_ice_user(ice_user));

        if has_traffic_callback {
            self.0
                .peers_traffic_watcher
                .register_peer(
                    self.0.room_id.clone(),
                    peer_id,
                    is_force_relayed,
                )
                .await
                .map_err(RoomError::PeerTrafficWatcherMailbox)
        } else {
            Ok(())
        }
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
    pub async fn connect_endpoints(
        self,
        src: WebRtcPublishEndpoint,
        sink: WebRtcPlayEndpoint,
    ) -> Result<ConnectEndpointsResult, RoomError> {
        use ConnectEndpointsResult::{Created, NoOp, Updated};

        debug!(
            "Connecting endpoints of Member [id = {}] with Member [id = {}]",
            src.owner().id(),
            sink.owner().id(),
        );
        let get_or_create_peers =
            self.clone().get_or_create_peers(src.clone(), sink.clone());
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
                        self.0.peers.take_inner_peer(src_peer_id).unwrap();
                    let mut sink_peer: Peer<Stable> =
                        self.0.peers.take_inner_peer(sink_peer_id).unwrap();

                    src_peer.add_publisher(
                        &src,
                        &mut sink_peer,
                        &self.0.tracks_count,
                    );

                    if src.has_traffic_callback() {
                        futs.push(self.0.peers_traffic_watcher.register_peer(
                            self.0.room_id.clone(),
                            src_peer_id,
                            src.is_force_relayed(),
                        ));
                    }
                    if sink.has_traffic_callback() {
                        futs.push(self.0.peers_traffic_watcher.register_peer(
                            self.0.room_id.clone(),
                            sink_peer_id,
                            sink.is_force_relayed(),
                        ));
                    }

                    sink_peer.add_endpoint(&sink.into());
                    src_peer.add_endpoint(&src.into());

                    let src_peer = PeerStateMachine::from(src_peer);
                    let sink_peer = PeerStateMachine::from(sink_peer);

                    self.0
                        .peer_metrics_service
                        .borrow_mut()
                        .update_peer_tracks(&src_peer);
                    self.0
                        .peer_metrics_service
                        .borrow_mut()
                        .update_peer_tracks(&sink_peer);

                    self.0.peers.add_peer(src_peer);
                    self.0.peers.add_peer(sink_peer);

                    future::try_join_all(futs)
                        .await
                        .map_err(RoomError::PeerTrafficWatcherMailbox)?;

                    Ok(Updated(src_peer_id, sink_peer_id))
                }
            }
        }
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
        self.0.peers.remove_peers_related_to_member(member_id)
    }

    /// Adds new [`WebRtcPlayEndpoint`] to the [`Peer`] with a provided
    /// [`PeerId`].
    pub fn add_sink(&self, peer_id: PeerId, sink: WebRtcPlayEndpoint) {
        let mut peer: Peer<Stable> = self.take_inner_peer(peer_id).unwrap();
        let mut partner_peer: Peer<Stable> =
            self.take_inner_peer(peer.partner_peer_id()).unwrap();

        peer.add_publisher(
            &sink.src(),
            &mut partner_peer,
            &self.0.tracks_count,
        );
        peer.add_endpoint(&Endpoint::from(sink));

        self.0.peers.add_peer(peer);
        self.0.peers.add_peer(partner_peer);
    }

    /// Updates [`PeerTracks`] of the [`Peer`] with provided [`PeerId`] in the
    /// [`PeerMetricsService`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    pub fn sync_peer_spec(&mut self, peer_id: PeerId) -> Result<(), RoomError> {
        self.0.peers.map_peer_by_id(peer_id, |peer| {
            self.0
                .peer_metrics_service
                .borrow_mut()
                .update_peer_tracks(&peer);
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
    use crate::api::control::endpoints::webrtc_publish_endpoint::{
        AudioSettings, VideoSettings,
    };

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

        let peers_service = PeersService::new(
            "test".into(),
            new_turn_auth_service_mock(),
            Arc::new(mock),
            &conf::Media::default(),
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

        peers_service
            .clone()
            .connect_endpoints(publish, play)
            .await
            .unwrap();

        register_peer_done.await.unwrap().unwrap();

        assert!(peers_service
            .0
            .peer_metrics_service
            .borrow()
            .is_peer_registered(PeerId(0)));
        assert!(peers_service
            .0
            .peer_metrics_service
            .borrow()
            .is_peer_registered(PeerId(1)));
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

        let peers_service = PeersService::new(
            "test".into(),
            new_turn_auth_service_mock(),
            Arc::new(mock),
            &conf::Media::default(),
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

        peers_service
            .clone()
            .connect_endpoints(publish, play)
            .await
            .unwrap();

        let first_peer_tracks_count = peers_service
            .0
            .peer_metrics_service
            .borrow()
            .peer_tracks_count(PeerId(0));
        assert_eq!(first_peer_tracks_count, 2);
        let second_peer_tracks_count = peers_service
            .0
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

        peers_service
            .clone()
            .connect_endpoints(publish, play)
            .await
            .unwrap();

        let first_peer_tracks_count = peers_service
            .0
            .peer_metrics_service
            .borrow()
            .peer_tracks_count(PeerId(0));
        assert_eq!(first_peer_tracks_count, 4);
        let second_peer_tracks_count = peers_service
            .0
            .peer_metrics_service
            .borrow()
            .peer_tracks_count(PeerId(1));
        assert_eq!(second_peer_tracks_count, 4);

        register_peer_done.await.unwrap();
    }
}
