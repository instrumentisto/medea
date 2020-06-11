//! Repository that stores [`Room`]s [`Peer`]s.

mod media_traffic_state;
mod metrics;
mod traffic_watcher;

use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use actix::{
    fut,
    fut::{wrap_future, Either},
    ActorFuture, WrapFuture as _,
};
use derive_more::Display;
use medea_client_api_proto::{
    AudioSettings, Incrementable, MediaType, PeerId, TrackId, VideoSettings,
};

use crate::{
    api::control::{MemberId, RoomId},
    conf,
    log::prelude::*,
    media::{
        peer::RenegotiationReason, IceUser, MediaTrack, Peer, PeerError,
        PeerStateMachine, Stable, WaitLocalSdp,
    },
    signalling::{
        elements::endpoints::{
            webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            Endpoint,
        },
        room::{ActFuture, RoomError},
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

    /// [`PeerTrafficWatcher`] which analyzes [`Peer`]s traffic metrics.
    peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,

    /// Service which responsible for this [`Room`]'s [`RtcStat`]s processing.
    peer_metrics_service: PeersMetricsService,

    /// Duration, after which [`Peer`]s stats will be considered as stale.
    /// Passed to [`PeersMetricsService`] when registering new [`Peer`]s.
    peer_stats_ttl: Duration,
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

/// Result of the [`PeersService::get_or_create_peers`] function.
#[derive(Debug, Clone, Copy)]
pub enum GetOrCreatePeersResult {
    /// Requested [`Peer`] pair was created.
    Created(PeerId, PeerId),

    /// Requested [`Peer`] pair already existed.
    AlreadyExisted(PeerId, PeerId),
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
        Self {
            room_id: room_id.clone(),
            turn_service,
            peers: HashMap::new(),
            peers_count: Counter::default(),
            tracks_count: Counter::default(),
            peers_traffic_watcher: peers_traffic_watcher.clone(),
            peer_metrics_service: PeersMetricsService::new(
                room_id,
                peers_traffic_watcher,
            ),
            peer_stats_ttl: media_conf.max_lag,
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
    pub fn remove_peers<'a, Peers: IntoIterator<Item = &'a PeerId>>(
        &mut self,
        member_id: &MemberId,
        peer_ids: Peers,
    ) -> HashMap<MemberId, Vec<PeerStateMachine>> {
        let mut removed_peers = HashMap::new();
        for peer_id in peer_ids {
            if let Some(peer) = self.peers.remove(&peer_id) {
                let partner_peer_id = peer.partner_peer_id();
                let partner_member_id = peer.partner_member_id();
                if let Some(partner_peer) = self.peers.remove(&partner_peer_id)
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
        src: WebRtcPublishEndpoint,
        sink: WebRtcPlayEndpoint,
    ) -> ActFuture<Result<GetOrCreatePeersResult, RoomError>> {
        Box::new(fut::ok::<(), (), Room>(()).then(move |_, room, _| {
            match room.peers.get_peers_between_members(
                &src.owner().id(),
                &sink.owner().id(),
            ) {
                None => {
                    let (src_peer_id, sink_peer_id) =
                        room.peers.create_peers(&src, &sink);

                    Either::Left(
                        room.peers
                            .peer_post_construct(src_peer_id, &sink.into())
                            .then(move |res, room, _| match res {
                                Ok(_) => Box::new(
                                    room.peers
                                        .peer_post_construct(
                                            sink_peer_id,
                                            &src.into(),
                                        )
                                        .map(move |res, _, _| {
                                            res.map(|_| {
                                                GetOrCreatePeersResult::Created(
                                                    src_peer_id,
                                                    sink_peer_id,
                                                )
                                            })
                                        }),
                                ),
                                Err(err) => {
                                    Box::new(fut::err(err)) as ActFuture<_>
                                }
                            }),
                    )
                }
                Some((first_peer_id, second_peer_id)) => {
                    Either::Right(fut::ok::<_, RoomError, Room>(
                        GetOrCreatePeersResult::AlreadyExisted(
                            first_peer_id,
                            second_peer_id,
                        ),
                    ))
                }
            }
        }))
    }

    /// Creates and sets [`IceUser`], registers [`Peer`] in
    /// [`PeerTrafficWatcher`].
    fn peer_post_construct(
        &self,
        peer_id: PeerId,
        endpoint: &Endpoint,
    ) -> ActFuture<Result<(), RoomError>> {
        let room_id = self.room_id.clone();
        let turn_service = self.turn_service.clone();
        let has_traffic_callback = endpoint.has_traffic_callback();
        let is_force_relayed = endpoint.is_force_relayed();
        Box::new(
            wrap_future(async move {
                Ok(turn_service
                    .create(room_id, peer_id, UnreachablePolicy::ReturnErr)
                    .await?)
            })
            .map(move |res: Result<IceUser, RoomError>, room: &mut Room, _| {
                res.map(|ice_user| {
                    if let Ok(peer) = room.peers.get_mut_peer_by_id(peer_id) {
                        peer.set_ice_user(ice_user)
                    }
                })
            })
            .then(move |res, room: &mut Room, _| {
                let room_id = room.id().clone();
                let traffic_watcher = room.peers.peers_traffic_watcher.clone();
                async move {
                    match res {
                        Ok(_) => {
                            if has_traffic_callback {
                                traffic_watcher
                                    .register_peer(
                                        room_id,
                                        peer_id,
                                        is_force_relayed,
                                    )
                                    .await
                                    .map_err(
                                        RoomError::PeerTrafficWatcherMailbox,
                                    )
                            } else {
                                Ok(())
                            }
                        }
                        Err(err) => Err(err),
                    }
                }
                .into_actor(room)
            }),
        )
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
        src: WebRtcPublishEndpoint,
        sink: WebRtcPlayEndpoint,
    ) -> ActFuture<Result<Option<(PeerId, PeerId)>, RoomError>> {
        debug!(
            "Connecting endpoints of Member [id = {}] with Member [id = {}]",
            src.owner().id(),
            sink.owner().id(),
        );
        Box::new(Self::get_or_create_peers(src.clone(), sink.clone()).map(
            |peers_res, room, _| {
                match peers_res? {
                    GetOrCreatePeersResult::Created(
                        src_peer_id,
                        sink_peer_id,
                    ) => {
                        let src_peer: Peer<Stable> =
                            room.peers.take_inner_peer(src_peer_id)?;
                        let sink_peer: Peer<Stable> =
                            room.peers.take_inner_peer(sink_peer_id)?;
                        let src_peer = PeerStateMachine::from(src_peer);
                        let sink_peer = PeerStateMachine::from(sink_peer);

                        room.peers.peer_metrics_service.register_peer(
                            &src_peer,
                            room.peers.peer_stats_ttl,
                        );
                        room.peers.peer_metrics_service.register_peer(
                            &sink_peer,
                            room.peers.peer_stats_ttl,
                        );

                        room.peers.add_peer(src_peer);
                        room.peers.add_peer(sink_peer);

                        Ok(Some((src_peer_id, sink_peer_id)))
                    }
                    GetOrCreatePeersResult::AlreadyExisted(
                        src_peer_id,
                        sink_peer_id,
                    ) => {
                        // TODO: here we assume that peers are stable,
                        //       which might not be the case, e.g. Control
                        //       Service creates multiple endpoints in quick
                        //       succession.
                        let mut src_peer: Peer<Stable> =
                            room.peers.take_inner_peer(src_peer_id).unwrap();
                        let mut sink_peer: Peer<Stable> =
                            room.peers.take_inner_peer(sink_peer_id).unwrap();

                        src_peer.add_publisher(
                            &mut sink_peer,
                            room.peers.get_tracks_counter(),
                        );

                        sink_peer.add_endpoint(&sink.into());
                        src_peer.add_endpoint(&src.into());

                        let src_peer = PeerStateMachine::from(src_peer);
                        let sink_peer = PeerStateMachine::from(sink_peer);

                        room.peers
                            .peer_metrics_service
                            .update_peer_tracks(&src_peer);
                        room.peers
                            .peer_metrics_service
                            .update_peer_tracks(&sink_peer);

                        room.peers.add_peer(src_peer);
                        room.peers.add_peer(sink_peer);

                        Ok(None)
                    }
                }
            },
        ))
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
            .for_each(|id| {
                self.peers.remove(id);
            });

        peers_to_remove
    }

    /// Starts renegotiation for a [`Peer`] with a provided [`PeerId`].
    ///
    /// # Panics
    ///
    /// If inserted `Peer` in [`WaitLocalSdp`] state isn't in this state.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::PeerNotFound`] if requested [`PeerId`] doesn't
    /// exist in [`PeerRepository`].
    ///
    /// Errors with [`RoomError::PeerError`] if [`Peer`] is found, but not in
    /// requested state.
    pub fn start_renegotiation(
        &mut self,
        peer_id: PeerId,
    ) -> Result<&mut Peer<WaitLocalSdp>, RoomError> {
        let peer: Peer<Stable> = self.take_inner_peer(peer_id)?;

        let renegotiating_peer =
            peer.start_renegotiation(RenegotiationReason::TracksAdded);
        let renegotiating_peer_id = renegotiating_peer.id();
        self.peers
            .insert(renegotiating_peer_id, renegotiating_peer.into());
        match self.get_mut_peer_by_id(renegotiating_peer_id)? {
            PeerStateMachine::WaitLocalSdp(peer) => Ok(peer),
            _ => unreachable!(
                "Peer with WaitLocalSdp state was inserted into the \
                 PeerService store, but different state was gotten."
            ),
        }
    }

    /// Adds new [`WebRtcPlayEndpoint`] to the [`Peer`] with a provided
    /// [`PeerId`].
    pub fn add_sink(&mut self, peer_id: PeerId, sink: WebRtcPlayEndpoint) {
        let mut peer: Peer<Stable> = self.take_inner_peer(peer_id).unwrap();
        let mut partner_peer: Peer<Stable> =
            self.take_inner_peer(peer.partner_peer_id()).unwrap();

        peer.add_endpoint(&sink.into());

        self.peers.insert(peer.id(), peer.into());
        self.peers.insert(partner_peer.id(), partner_peer.into());
    }
}

// TODO: so there is few errors here:
//      1. peers_traffic_watcher.register_peer is called only after peer is
//         created, and not called when we add new endpoints.
//      2. peer_metrics_service.register_peer wasn't called anywhere
//      so its time to cover this with unit tests:
//      1. make sure that peer is registered in metrics service when it is
// created      2. make sure that peer is update in metrics service when new
// endpoint is added      3. peer registration in peers_traffic_watcher is
// working as expected      4. peer state changes as expected after new endpoint
// was added
