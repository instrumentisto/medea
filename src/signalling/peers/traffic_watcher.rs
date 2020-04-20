//! Provides [`PeerTrafficWatcher`] trait and its impl.
//!
//! [`PeerTrafficWatcher`] analyzes [`Peer`] traffic metrics and send messages
//! ([`PeerStarted`], [`PeerStopped`], [`PeerFailed`]) to [`Room`].
//!
//! Traffic metrics, consumed by [`PeerTrafficWatcher`] can originate from
//! different sources:
//!
//! 1. [`FlowMetricSource::Peer`] - Peer, received from member that owns target
//!    [`Peer`].
//!
//! 2. [`FlowMetricSource::PartnerPeer`] - Partner Peer, received from member,
//!    that owns [`Peer`], connected to target [`Peer`].
//!
//! 3. [`FlowMetricSource::Coturn`] - Coturn, reported by Coturn TURN server.
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

use crate::{api::control::RoomId, conf, log::prelude::*, signalling::Room};

/// Returns [`FlowMetricSources`], which will be used to emit [`Peer`] state
/// events.
///
/// [`FlowMetricSource::Peer`] and [`FlowMetricSource::PartnerPeer`] are
/// always returned, [`FlowMetricSource::Coturn`] is optional (should be used
/// only if media is forcibly relayed).
fn build_flow_sources(should_watch_turn: bool) -> HashSet<FlowMetricSource> {
    let mut sources = HashSet::new();
    sources.insert(FlowMetricSource::Peer);
    sources.insert(FlowMetricSource::PartnerPeer);
    if should_watch_turn {
        sources.insert(FlowMetricSource::Coturn);
    }

    sources
}

/// Builds [`PeerTrafficWatcher`] backed by [`PeersTrafficWatcherImpl`] actor.
pub fn build_peers_traffic_watcher(
    conf: &conf::PeerMediaTraffic,
) -> Arc<dyn PeerTrafficWatcher> {
    Arc::new(PeersTrafficWatcherImpl::new(conf).start())
}

/// Message which indicates that [`Peer`] with provided [`PeerId`]
/// has started.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct PeerStarted(pub PeerId);

/// Message which indicates that [`Peer`] with provided [`PeerId`]
/// has stopped.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct PeerStopped(pub PeerId);

/// Message which indicates that [`Peer`] with provided [`PeerId`] was fallen
/// into failure state and should be removed.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct FatalPeerFailure {
    /// [`PeerId`] of [`Peer`] which went into error state.
    pub peer_id: PeerId,

    /// The [`DateTime`] from which it is believed that [`Peer`] went into an
    /// erroneous state.
    pub at: DateTime<Utc>,
}

/// Consumes [`Peer`] traffic metrics for further processing.
#[async_trait]
pub trait PeerTrafficWatcher: Debug + Send + Sync {
    /// Registers [`Room`] as [`Peer`]'s state messages listener, preparing
    /// [`PeerTrafficWatcher`] for registering [`Peer`]s from this [`Room`].
    async fn register_room(
        &self,
        room_id: RoomId,
        room: WeakAddr<Room>,
    ) -> Result<(), MailboxError>;

    /// Unregisters [`Room`] as [`Peer`]'s state messages listener.
    ///
    /// All [`Peer`] subscriptions related to this [`Room`] will be removed.
    fn unregister_room(&self, room_id: RoomId);

    /// Registers [`Peer`], so that [`PeerTrafficWatcher`] will be able to
    /// process traffic flow events of this [`Peer`].
    async fn register_peer(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        should_watch_turn: bool,
    ) -> Result<(), MailboxError>;

    /// Unregisters [`Peer`]s, so that [`PeerTrafficWatcher`] will not be able
    /// to process traffic flow events of this [`Peer`] anymore.
    fn unregister_peers(&self, room_id: RoomId, peers_ids: HashSet<PeerId>);

    /// Notifies [`PeerTrafficWatcher`] that some [`Peer`] traffic flowing.
    fn traffic_flows(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        at: Instant,
        source: FlowMetricSource,
    );

    /// Notifies [`PeerTrafficWatcher`] that some [`Peer`]'s traffic flowing was
    /// stopped.
    fn traffic_stopped(&self, room_id: RoomId, peer_id: PeerId, at: Instant);
}

#[async_trait]
impl PeerTrafficWatcher for Addr<PeersTrafficWatcherImpl> {
    /// Sends [`RegisterRoom`] message to the [`PeersTrafficWatcherImpl`]
    /// returning send result.
    async fn register_room(
        &self,
        room_id: RoomId,
        room: WeakAddr<Room>,
    ) -> Result<(), MailboxError> {
        self.send(RegisterRoom { room_id, room }).await
    }

    /// Sends [`UnregisterRoom`] message to [`PeersTrafficWatcherImpl`].
    fn unregister_room(&self, room_id: RoomId) {
        self.do_send(UnregisterRoom(room_id))
    }

    /// Sends [`RegisterPeer`] message to [`PeersTrafficWatcherImpl`] returning
    /// send result.
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

    /// Sends [`UnregisterPeers`] message to [`PeersTrafficWatcherImpl`].
    fn unregister_peers(
        &self,
        room_id: RoomId,
        peers_ids: HashSet<PeerId, RandomState>,
    ) {
        self.do_send(UnregisterPeers { room_id, peers_ids })
    }

    /// Sends [`TrafficFlows`] message to [`PeersTrafficWatcherImpl`].
    fn traffic_flows(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        at: Instant,
        source: FlowMetricSource,
    ) {
        debug!("TrafficFlows: in {}/{} from {:?}", room_id, peer_id, source);
        self.do_send(TrafficFlows {
            room_id,
            peer_id,
            at,
            source,
        })
    }

    /// Sends [`TrafficStopped`] message to [`PeersTrafficWatcherImpl`].
    fn traffic_stopped(&self, room_id: RoomId, peer_id: PeerId, at: Instant) {
        debug!("TrafficStopped: in {}/{}", room_id, peer_id);
        self.do_send(TrafficStopped {
            room_id,
            peer_id,
            at,
        })
    }
}

/// Service which analyzes [`Peer`] traffic metrics and send messages
/// ([`PeerStarted`], [`PeerStopped`], [`PeerFailed`]) to [`Room`].
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug, Default)]
struct PeersTrafficWatcherImpl {
    /// All `Room`s which exists on the Medea server.
    stats: HashMap<RoomId, RoomStats>,

    /// Duration after which media server will consider that `Peer` stopped.
    traffic_flowing_timeout: Duration,

    /// Duration within which media server should receive `Peer` start signal
    /// from all sources.
    peer_init_timeout: Duration,
}

impl PeersTrafficWatcherImpl {
    /// Returns new [`PeersTrafficWatcherImpl`].
    pub fn new(conf: &conf::PeerMediaTraffic) -> Self {
        Self {
            stats: HashMap::new(),
            traffic_flowing_timeout: conf.traffic_flowing_timeout,
            peer_init_timeout: conf.peer_init_timeout,
        }
    }

    /// Unsubscribes [`PeersTrafficWatcherImpl`] from watching a
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

    /// Unsubscribes [`PeersTrafficWatcherImpl`] from a [`Peer`] with
    /// [`FatalPeerFailure`] error and notifies [`Room`] about this.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    fn peer_init_error(
        &mut self,
        room_id: &RoomId,
        peer_id: PeerId,
        at: DateTime<Utc>,
    ) {
        if let Some(room) = self.stats.get_mut(&room_id) {
            room.peers.remove(&peer_id);
            if let Some(room_addr) = room.room.upgrade() {
                room_addr.do_send(FatalPeerFailure { peer_id, at });
            }
        }
    }

    /// Checks that all [`FlowMetricSource`] considered that [`Peer`] with
    /// provided [`PeerId`] is started.
    ///
    /// This function will be called on every [`PeerStat`] after
    /// [`PeerTrafficWatcherImpl::peer_init_timeout`] time from
    /// first [`PeerStat`]'s [`TrafficFlows`] message.
    ///
    /// If this check fails then [`PeersTrafficWatcher::peer_init_error`]
    /// will be called for this [`PeerStat`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    fn check_on_start(&mut self, room_id: &RoomId, peer_id: PeerId) {
        let peer = self
            .stats
            .get_mut(room_id)
            .and_then(|room| room.peers.get_mut(&peer_id));

        if let Some(peer) = peer {
            let srcs = if let PeerState::Starting(srcs) = &peer.state {
                if srcs.len() < peer.flow_metrics_sources.len() {
                    let started_at = peer.started_at.unwrap_or_else(Utc::now);
                    self.peer_init_error(room_id, peer_id, started_at);
                    return;
                }
                srcs.iter().map(|src| (*src, Instant::now())).collect()
            } else {
                return;
            };
            peer.state = PeerState::Started(srcs);
        }
    }
}

impl Actor for PeersTrafficWatcherImpl {
    type Context = actix::Context<Self>;

    /// Starts stats watchdog which will check every second that all traffic
    /// flowing. If some traffic flowing was stopped then [`TrafficStopped`]
    /// event will be sent to this [`PeersTrafficWatcherImpl`].
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_secs(1), |this, ctx| {
            for stat in this.stats.values() {
                for peer in stat.peers.values() {
                    if !peer.is_valid(this.traffic_flowing_timeout) {
                        ctx.notify(TrafficStopped {
                            peer_id: peer.peer_id,
                            room_id: stat.room_id.clone(),
                            at: Instant::now(),
                        });
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

impl Handler<TrafficFlows> for PeersTrafficWatcherImpl {
    type Result = ();

    /// Updates [`PeerStat::last_update`] time.
    ///
    /// If [`PeerStat`] in [`PeerState::Stopped`] state then this stat will
    /// be flowed into [`PeerState::Starting`] state in which [`Peer`] init
    /// check should be performed. Also [`PeersTrafficWatcherImpl::
    /// check_on_start`] function will be called after
    /// [`PeersTrafficWatcherImpl::peer_init_timeout`] time.
    ///
    /// If [`PeerStat`] in [`PeerState::Starting`] then provided
    /// [`FlowMetricSource`] will be inserted into list of received
    /// [`FlowMetricSource`]. This list will be checked in the
    /// [`PeersTrafficWatcherImpl::check_on_start`] function.
    ///
    /// If [`PeerStat`] in [`PeerState::Started`] then last update time of the
    /// provided [`FlowMetricSource`] will be updated.
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
                        sources.insert(msg.source, Instant::now());
                    }
                    PeerState::Starting(sources) => {
                        sources.insert(msg.source);
                    }
                    PeerState::NotStarted => {
                        let mut srcs = HashSet::new();
                        srcs.insert(msg.source);
                        peer.state = PeerState::Starting(srcs);
                        peer.started_at = Some(Utc::now());

                        ctx.run_later(
                            self.peer_init_timeout,
                            move |this, _| {
                                this.check_on_start(&msg.room_id, msg.peer_id);
                            },
                        );

                        if let Some(room_addr) = room.room.upgrade() {
                            room_addr.do_send(PeerStarted(peer.peer_id));
                        }
                    }
                    PeerState::Stopped(received_flow_sources) => {
                        received_flow_sources.insert(msg.source);
                        if *received_flow_sources == peer.flow_metrics_sources {
                            peer.state = PeerState::Started(
                                received_flow_sources
                                    .iter()
                                    .map(|src| (*src, Instant::now()))
                                    .collect(),
                            );
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
}

impl Handler<TrafficStopped> for PeersTrafficWatcherImpl {
    type Result = ();

    /// Removed subscription on the [`Peer`] stats with provided [`PeerStat`]
    /// from the [`PeersTrafficWatcherImpl`], sends [`PeerStopped`] into
    /// [`Room`].
    fn handle(
        &mut self,
        msg: TrafficStopped,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.peers.get_mut(&msg.peer_id) {
                if let PeerState::Stopped(_) = &peer.state {
                } else {
                    peer.state = PeerState::Stopped(HashSet::new());
                    if let Some(room_addr) = room.room.upgrade() {
                        room_addr.do_send(PeerStopped(peer.peer_id));
                    }
                }
            }
        }
    }
}

/// All sources of [`TrafficFlows`] message.
///
/// This is needed for checking that all metrics sources have the same opinion
/// about current [`Peer`]'s traffic state.
///
/// [`PeersTrafficWatcher`] checks that all sources have the same opinion
/// after [`PeersTrafficWatcherImpl::peer_init_timeout`] from first
/// [`TrafficFlows`] message received for some [`PeerStat`]. If at least one
/// [`FlowMetricSource`] doesn't sent [`TrafficFlows`] message, then [`Peer`]
/// will be considered as wrong and it will be stopped.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum FlowMetricSource {
    /// Metrics from the partner [`Peer`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    PartnerPeer,

    /// Metrics from the [`Peer`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    Peer,

    /// Metrics for this [`Peer`] from the Coturn TURN server.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    Coturn,
}

/// Current state of [`PeerStat`].
///
/// If [`PeerStat`] goes into [`PeerState::Started`] then all
/// [`FlowMetricSource`]s should notify [`PeersTrafficWatcher`] about it.
#[derive(Debug)]
pub enum PeerState {
    /// [`PeerStat`] is started and [`TrafficFlows`] received from all
    /// [`FlowMetricSource`].
    Started(HashMap<FlowMetricSource, Instant>),

    /// First [`TrafficFlows`] received but not from all [`FlowMetricSource`]s.
    Starting(HashSet<FlowMetricSource>),

    /// [`Peer`] currently is not started, and waits for the first stats.
    NotStarted,

    Stopped(HashSet<FlowMetricSource>),
}

/// Current stats of [`Peer`].
///
/// Also this structure may be considered as subscription to this [`Peer`] state
/// updates.
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

    /// [`DateTime`] when this [`PeerStat`] is started.
    ///
    /// If `None` then [`PeerStat`] not started.
    started_at: Option<DateTime<Utc>>,

    /// Time of last received [`PeerState`] proof.
    ///
    /// If [`PeerStat`] doesn't updates withing
    /// [`PeerTrafficWatcherImpl::traffic_flowing_timeout`] then this
    /// [`PeerStat`] will be considered as stopped and will be removed.
    last_update: Instant,
}

impl PeerStat {
    /// Returns `true` if this [`PeerStat`] is considered as valid.
    ///
    /// Checks that [`TrafficFlows`] received from all [`FlowMetricSource`]s
    /// within provided `traffic_flowing_timeout`.
    pub fn is_valid(&self, traffic_flowing_timeout: Duration) -> bool {
        if let PeerState::Started(srcs) = &self.state {
            for src in &self.flow_metrics_sources {
                if let Some(src_last_update) = srcs.get(src) {
                    if src_last_update.elapsed() > traffic_flowing_timeout {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            if self.last_update.elapsed() > traffic_flowing_timeout {
                return false;
            }
        }

        true
    }
}

/// Stores [`PeerStat`]s of [`Peer`]s for which stats updates [`Room`]
/// is watching.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug)]
pub struct RoomStats {
    /// [`RoomId`] of all [`PeerStat`] which stored here.
    room_id: RoomId,

    /// [`Addr`] of [`Room`] which is watching for this [`PeerStat`]s.
    room: WeakAddr<Room>,

    /// [`PeerStat`] for which [`Room`] is watching.
    peers: HashMap<PeerId, PeerStat>,
}

/// Registers new [`Room`] as [`PeerStat`]s listener.
///
/// This message will only add provided [`Room`] to the list. For real
/// subscription to a [`PeerStat`] [`RegisterPeer`] message should be used.
///
/// [`RegisterRoom`] will be called in [`RoomService`] for every [`Room`] when
/// it created.
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct RegisterRoom {
    /// [`RoomId`] of [`Room`] which requested to register in the
    /// [`PeersTrafficWatcherImpl`].
    room_id: RoomId,

    /// [`Addr`] of [`Room`] which requested to register in the
    /// [`PeersTrafficWatcherImpl`].
    room: WeakAddr<Room>,
}

impl Handler<RegisterRoom> for PeersTrafficWatcherImpl {
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

impl Handler<UnregisterRoom> for PeersTrafficWatcherImpl {
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

impl Handler<RegisterPeer> for PeersTrafficWatcherImpl {
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
                    state: PeerState::NotStarted,
                    flow_metrics_sources: msg.flow_metrics_sources,
                    last_update: Instant::now(),
                    started_at: None,
                },
            );
        }
    }
}

/// Unregisters [`Room`] with provided [`RoomId`] from [`PeerStat`] with
/// provided [`PeerId`] updates receiving.
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct UnregisterPeers {
    /// [`RoomId`] of [`Room`] which unregisters from [`PeerStat`]'s
    /// [`PeerState`] changes.
    room_id: RoomId,

    /// [`PeerId`] of [`PeerStat`] from which unregistration is requested.
    peers_ids: HashSet<PeerId>,
}

impl Handler<UnregisterPeers> for PeersTrafficWatcherImpl {
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
