//! Service which is responsible to [`Peer`] metrics based Control API
//! callbacks.
//!
//! [`Peer`] metrics will be collected from the many sources of
//! metrics. All this metrics will be collected and based on them,
//! [`PeersTrafficWatcher`] will consider which callback should be sent (or
//! not sent).
//!
//! # List of [`Peer`] metrics based Control API callbacks:
//!
//! 1. `WebRtcPublishEndpoint::on_start`;
//! 2. `WebRtcPublishEndpoint::on_stop`;
//! 3. `WebRtcPlayEndpoint::on_start`;
//! 4. `WebRtcPlayEndpoint::on_stop`.
//!
//! [`Peer`]: crate::media::peer::Peer

use std::{
    collections::{hash_map::RandomState, HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
    time::{Duration, Instant},
};

use actix::{
    Actor, Addr, AsyncContext, Handler, MailboxError, Message, WeakAddr,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use medea_client_api_proto::PeerId;

use crate::{
    api::control::RoomId,
    signalling::{
        room::{PeerFailed, PeerStarted, PeerStopped},
        Room,
    },
};

/// Returns [`FlowMetricSources`] which should be used to validate that
/// `Endpoint` is started based on `force_relay` property from Control API spec.
fn build_flow_sources(is_force_relay: bool) -> HashSet<FlowMetricSource> {
    // This code is needed to pay attention to this function when changing
    // 'FlowMetricSource'.
    //
    // Rustc shouldn't include it into binary.
    {
        match FlowMetricSource::PeerTraffic {
            FlowMetricSource::PeerTraffic
            | FlowMetricSource::Coturn
            | FlowMetricSource::PartnerPeerTraffic => (),
        }
    }

    let mut sources = HashSet::new();
    sources.insert(FlowMetricSource::PeerTraffic);
    sources.insert(FlowMetricSource::PartnerPeerTraffic);
    if is_force_relay {
        sources.insert(FlowMetricSource::Coturn);
    }

    sources
}

pub fn build_peers_traffic_watcher() -> Arc<dyn PeerTrafficWatcher> {
    Arc::new(InnerPeersTrafficWatcher::new().start())
}

#[async_trait]
pub trait PeerTrafficWatcher: Debug + Send + Sync {
    async fn register_room(
        &self,
        room_id: RoomId,
        room: WeakAddr<Room>,
    ) -> Result<(), MailboxError>;

    fn unregister_room(&self, room_id: RoomId);

    async fn register_peer(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        should_watch_turn: bool,
    ) -> Result<(), MailboxError>;

    fn unregister_peers(&self, room_id: RoomId, peers_ids: HashSet<PeerId>);

    fn traffic_flows(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        at: Instant,
        source: FlowMetricSource,
    );

    fn traffic_stopped(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        at: Instant,
        source: StoppedMetricSource,
    );
}

#[async_trait]
impl PeerTrafficWatcher for Addr<InnerPeersTrafficWatcher> {
    async fn register_room(
        &self,
        room_id: RoomId,
        room: WeakAddr<Room>,
    ) -> Result<(), MailboxError> {
        self.send(RegisterRoom { room_id, room }).await
    }

    fn unregister_room(&self, room_id: RoomId) {
        self.do_send(UnregisterRoom(room_id))
    }

    async fn register_peer(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        should_watch_turn: bool,
    ) -> Result<(), MailboxError> {
        self.send(RegisterPeer {
            room_id,
            peer_id,
            flow_metrics_sources: build_flow_sources(should_watch_turn),
        })
        .await
    }

    fn unregister_peers(
        &self,
        room_id: RoomId,
        peers_ids: HashSet<PeerId, RandomState>,
    ) {
        self.do_send(UnregisterPeers { room_id, peers_ids })
    }

    fn traffic_flows(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        at: Instant,
        source: FlowMetricSource,
    ) {
        self.do_send(TrafficFlows {
            room_id,
            peer_id,
            at,
            source,
        })
    }

    fn traffic_stopped(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        at: Instant,
        source: StoppedMetricSource,
    ) {
        self.do_send(TrafficStopped {
            room_id,
            peer_id,
            at,
            source,
        })
    }
}

/// Service which responsible for the [`Peer`] metrics based Control
/// API callbacks.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug, Default)]
struct InnerPeersTrafficWatcher {
    /// All `Room` which exists on the Medea server.
    stats: HashMap<RoomId, RoomStats>,
}

impl InnerPeersTrafficWatcher {
    /// Returns new [`MetricsCallbacksService`].
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    /// Unsubscribes [`PeersTrafficWatcher`] from watching a
    /// [`Peer`] with provided [`PeerId`].
    ///
    /// Removes provided [`PeerId`] from [`RoomStat`] with provided [`RoomId`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    pub fn unsubscribe_from_peer(&mut self, room_id: &RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(room_id) {
            room.peers.remove(&peer_id);
        }
    }

    /// Unsubscribes [`PeersTrafficWatcher`] from a [`Peer`] with
    /// fatal error and notifies [`Room`] about fatal error in [`PeerStat`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    fn fatal_peer_error(
        &mut self,
        room_id: &RoomId,
        peer_id: PeerId,
        at: DateTime<Utc>,
    ) {
        if let Some(room) = self.stats.get_mut(&room_id) {
            room.peers.remove(&peer_id);
            if let Some(room_addr) = room.room.upgrade() {
                room_addr.do_send(PeerFailed { peer_id, at });
            }
        }
    }

    /// Checks that all metrics sources considered that [`Peer`] with
    /// provided [`PeerId`] is started.
    ///
    /// This function will be called on every [`PeerStat`] after `10s` from
    /// first [`PeerStat`]'s [`TrafficFlows`] message.
    ///
    /// If this check fails then [`PeersTrafficWatcher::fatal_peer_error`]
    /// will be called for this [`PeerStat`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    fn check_on_start(&mut self, room_id: &RoomId, peer_id: PeerId) {
        let peer = self
            .stats
            .get_mut(room_id)
            .and_then(|room| room.peers.get_mut(&peer_id));

        if let Some(peer) = peer {
            if let PeerState::Started(srcs) = &peer.state {
                let is_not_all_sources_sent_start =
                    srcs.len() < peer.flow_metrics_sources.len();
                if is_not_all_sources_sent_start {
                    let started_at = peer.started_at.unwrap_or_else(Utc::now);
                    self.fatal_peer_error(room_id, peer_id, started_at);
                }
            }
        }
    }
}

impl Actor for InnerPeersTrafficWatcher {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_secs(10), |this, ctx| {
            for stat in this.stats.values() {
                for track in stat.peers.values() {
                    if let PeerState::Started(_) = &track.state {
                        if track.last_update
                            < Instant::now() - Duration::from_secs(10)
                        {
                            ctx.notify(TrafficStopped {
                                source: StoppedMetricSource::Timeout,
                                peer_id: track.peer_id,
                                room_id: stat.room_id.clone(),
                                at: Instant::now(),
                            });
                        }
                    }
                }
            }
        });
    }
}

/// Some [`FlowMetricSource`] notifies [`MetricsCallbacksService`] that
/// [`Peer`] with provided [`PeerId`] is normally flows.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct TrafficFlows {
    /// [`RoomId`] of [`Room`] where this [`Peer`] is stored.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    room_id: RoomId,

    /// [`PeerId`] of [`Peer`] which flows.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    peer_id: PeerId,

    /// Time when proof of [`Peer`]'s traffic flowing was gotten.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    at: Instant,

    /// Source of this metric.
    source: FlowMetricSource,
}

impl Handler<TrafficFlows> for InnerPeersTrafficWatcher {
    type Result = ();

    fn handle(
        &mut self,
        msg: TrafficFlows,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.peers.get_mut(&msg.peer_id) {
                peer.last_update = msg.at;
                match &mut peer.state {
                    PeerState::Started(sources) => {
                        sources.insert(msg.source);
                    }
                    PeerState::Stopped => {
                        let mut srcs = HashSet::new();
                        srcs.insert(msg.source);
                        peer.state = PeerState::Started(srcs);
                        peer.started_at = Some(Utc::now());

                        ctx.run_later(
                            Duration::from_secs(15),
                            move |this, _| {
                                this.check_on_start(&msg.room_id, msg.peer_id);
                            },
                        );

                        if let Some(room_addr) = room.room.upgrade() {
                            room_addr.do_send(PeerStarted(peer.peer_id));
                        }
                    }
                }
            }
        }
    }
}

/// Some [`StoppedMetricSource`] notifies [`MetricsCallbacksService`] that
/// traffic flowing of [`Peer`] with provided [`PeerId`] was stopped.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct TrafficStopped {
    /// [`RoomId`] of [`Room`] where this [`Peer`] is stored.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    room_id: RoomId,

    /// [`PeerId`] of [`Peer`] which traffic was stopped.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    peer_id: PeerId,

    /// Time when proof of [`Peer`]'s traffic stopping was gotten.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    at: Instant,

    /// Source of this metric.
    source: StoppedMetricSource,
}

impl Handler<TrafficStopped> for InnerPeersTrafficWatcher {
    type Result = ();

    fn handle(
        &mut self,
        msg: TrafficStopped,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.peers.remove(&msg.peer_id) {
                if let Some(room_addr) = room.room.upgrade() {
                    room_addr.do_send(PeerStopped(peer.peer_id));
                }
            }
        }
        self.unsubscribe_from_peer(&msg.room_id, msg.peer_id);
    }
}

/// All sources of [`TrafficFlows`] message.
///
/// This is needed for checking that all metrics sources have the same opinion
/// about current [`Peer`]'s traffic state.
///
/// [`PeersTrafficWatcher`] checks that all sources have the same opinion
/// after `10s` from first [`TrafficFlows`] message received for some
/// [`PeerStat`]. If at least one [`FlowMetricSource`] doesn't sent
/// [`TrafficFlows`] message, then [`Peer`] will be considered as wrong
/// and it will be stopped.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum FlowMetricSource {
    /// Metrics from the partner [`Peer`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    PartnerPeerTraffic,

    /// Metrics from the [`Peer`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    PeerTraffic,

    /// Metrics for this [`Peer`] from the Coturn TURN server.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    Coturn,
}

/// All sources of [`TrafficStopped`] message.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum StoppedMetricSource {
    /// [`Peer`] was removed.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    PeerRemoved,

    /// Partner [`Peer`] was removed.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    PartnerPeerRemoved,

    /// [`Peer`] traffic stopped growing.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    PeerTraffic,

    /// All Coturn allocations related to this [`Peer`] was removed.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    Coturn,

    /// [`PeersTrafficWatcher`] doesn't receive [`TrafficFlows`] too long.
    Timeout,
}

/// Current state of [`PeerStat`].
///
/// If [`PeerStat`] goes into [`PeerState::Started`] then all
/// [`FlowMetricSource`]s should notify [`PeersTrafficWatcher`] about it.
#[derive(Debug)]
pub enum PeerState {
    /// [`PeerStat`] is started.
    Started(HashSet<FlowMetricSource>),

    /// [`PeerStat`] is stopped.
    Stopped,
}

/// Current state of [`Peer`].
///
/// Also this structure may be considered as subscription to Control API
/// callbacks.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug)]
pub struct PeerStat {
    /// [`PeerId`] of [`Peer`] which this [`PeerStat`] represents.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    peer_id: PeerId,

    /// Current state of this [`PeerStat`].
    state: PeerState,

    /// List of [`FlowMetricSource`]s from which [`TrafficFlows`] should be
    /// received for validation that traffic is really going.
    flow_metrics_sources: HashSet<FlowMetricSource>,

    /// [`DateTime`] when this [`PeerStat`] went into [`PeerState::Started`]
    /// state.
    ///
    /// If `None` then [`PeerStat`] not in [`PeerState::Started`] state.
    started_at: Option<DateTime<Utc>>,

    /// Time of last received [`PeerState`] proof.
    ///
    /// If [`PeerStat`] doesn't updates withing `10s` then this [`PeerStat`]
    /// will be considered as [`PeerState::Stopped`].
    last_update: Instant,
}

/// Stores [`PeerStat`]s of [`Peer`]s for which [`PeerState`] [`Room`]
/// is watching.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug)]
pub struct RoomStats {
    /// [`RoomId`] of all [`PeerStat`] which stored here.
    room_id: RoomId,

    /// [`Addr`] of [`Room`] which is watching for this [`PeerStat`]s.
    room: WeakAddr<Room>,

    /// [`PeerStat`] for which some [`Room`] is watching.
    peers: HashMap<PeerId, PeerStat>,
}

/// Registers new [`Room`] as [`PeerStat`]s watcher.
///
/// This message will only add provided [`Room`] to the list. For real
/// subscription to a [`PeerStat`] [`Subscribe`] message should be used.
///
/// [`RegisterRoom`] will be called in [`RoomService`] for every [`Room`] when
/// it created.
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct RegisterRoom {
    /// [`RoomId`] of [`Room`] which requested to register in the
    /// [`MetricsCallbacksService`].
    room_id: RoomId,

    /// [`Addr`] of room which requested to register in the
    /// [`PeersTrafficWatcher`].
    room: WeakAddr<Room>,
}

impl Handler<RegisterRoom> for InnerPeersTrafficWatcher {
    type Result = ();

    fn handle(
        &mut self,
        msg: RegisterRoom,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.stats.insert(
            msg.room_id.clone(),
            RoomStats {
                room_id: msg.room_id,
                room: msg.room,
                peers: HashMap::new(),
            },
        );
    }
}

/// Unregisters [`Room`] with provided [`RoomId`] from the
/// [`MetricsCallbacksService`].
///
/// This message will just remove subscription. This isn't considered as
/// [`TrafficStopped`] or something like this.
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct UnregisterRoom(pub RoomId);

impl Handler<UnregisterRoom> for InnerPeersTrafficWatcher {
    type Result = ();

    fn handle(
        &mut self,
        msg: UnregisterRoom,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.stats.remove(&msg.0);
    }
}

/// Subscribes [`Room`] with provided [`RoomId`] to [`PeerStat`] with provided
/// [`PeerId`].
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct RegisterPeer {
    /// [`RoomId`] of [`Room`] which subscribes on [`PeerStat`]'s [`PeerState`]
    /// changes.
    room_id: RoomId,

    /// [`PeerId`] of [`PeerStat`] for which subscription is requested.
    peer_id: PeerId,

    /// List of [`FlowMetricSource`]s from which [`TrafficFlows`] should be
    /// received for validation that traffic is really going.
    flow_metrics_sources: HashSet<FlowMetricSource>,
}

impl Handler<RegisterPeer> for InnerPeersTrafficWatcher {
    type Result = ();

    fn handle(
        &mut self,
        msg: RegisterPeer,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            room.peers.insert(
                msg.peer_id,
                PeerStat {
                    peer_id: msg.peer_id,
                    state: PeerState::Stopped,
                    flow_metrics_sources: msg.flow_metrics_sources,
                    last_update: Instant::now(),
                    started_at: None,
                },
            );
        }
    }
}

/// Unsubscribes [`Room`] with provided [`RoomId`] from [`PeerStat`] with
/// provided [`PeerId`].
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct UnregisterPeers {
    /// [`RoomId`] of [`Room`] which unsubscribes from [`PeerStat`]'s
    /// [`PeerState`] changes.
    room_id: RoomId,

    /// [`PeerId`] of [`PeerStat`] from which unsubscription is requested.
    peers_ids: HashSet<PeerId>,
}

impl Handler<UnregisterPeers> for InnerPeersTrafficWatcher {
    type Result = ();

    fn handle(
        &mut self,
        msg: UnregisterPeers,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room_stats) = self.stats.get_mut(&msg.room_id) {
            for peer_id in msg.peers_ids {
                room_stats.peers.remove(&peer_id);
            }
        }
    }
}
