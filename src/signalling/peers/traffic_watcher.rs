//! Provides [`PeerTrafficWatcher`] trait and its impl.
//!
//! [`PeerTrafficWatcher`] analyzes [`Peer`] traffic metrics and send messages
//! ([`PeerStarted`], [`PeerStopped`]) to [`Room`].
//!
//! Traffic metrics, consumed by [`PeerTrafficWatcher`] can originate from
//! different sources:
//! 1. [`FlowMetricSource::Peer`] - Stats received from member that owns target
//!    [`Peer`].
//! 2. [`FlowMetricSource::PartnerPeer`] - Stats received from member,
//!    that owns [`Peer`], connected to target [`Peer`].
//! 3. [`FlowMetricSource::Coturn`] - Stats reported by Coturn TURN server, this
//! source is only being tracked if target [`Peer`] traffic is being relayed.
//!
//! At first you should register [`Room`] (`PeerTrafficWatcher.register_room()`)
//! and [`Peer`] (`PeerTrafficWatcher.register_peer()`). When first source will
//! report that traffic is flowing (`PeerTrafficWatcher.traffic_flows()`)
//! [`PeerStarted`] event will be sent to [`Room`].
//!
//! After that [`PeerTrafficWatcher`] will wait for other sources to report that
//! traffic is flowing for `peer_init_timeout`, or [`PeerStopped`] event will be
//! sent to [`Room`].
//!
//! If some source will report that it observes traffic stopped flowing
//! (`PeerTrafficWatcher.traffic_stopped()`), then [`PeerStopped`] message will
//! be sent to [`Room`].
//!
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
    let mut sources =
        hashset![FlowMetricSource::Peer, FlowMetricSource::PartnerPeer];
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
pub struct PeerStopped {
    pub peer_id: PeerId,
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
        source: FlowMetricSource,
    ) {
        debug!("TrafficFlows: in {}/{} from {:?}", room_id, peer_id, source);
        self.do_send(TrafficFlows {
            room_id,
            peer_id,
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
/// ([`PeerStarted`], [`PeerStopped`]) to [`Room`].
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug, Default)]
struct PeersTrafficWatcherImpl {
    /// All `Room`s which exists on the Medea server.
    stats: HashMap<RoomId, RoomStats>,

    /// Media source traffic report ttl. Media sources must continuously report
    /// that traffic is flowing, if some media source wont send new reports for
    /// this timeout, then it is considered that this source is not flowing any
    /// more.
    traffic_flowing_timeout: Duration,

    /// Duration after which [`PeersTrafficWatcherImpl`] will check that all
    /// tracked traffic sources have reported that traffic is flowing.
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

    /// Checks that all [`FlowMetricSource`] have reported that [`Peer`] traffic
    /// is flowing.
    ///
    /// If this check fails, then [`PeerStopped`] message is sent to [`Room`]
    /// with `at` field set at time, when first source reported that [`Peer`]
    /// traffic is flowing.
    ///
    /// Called for every [`Peer`] after
    /// `peer_init_timeout` passed since first source reported that [`Peer`]
    /// traffic is flowing.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    fn check_is_started(&mut self, room_id: &RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(room_id) {
            if let Some(peer) = room.peers.get_mut(&peer_id) {
                if peer.state == PeerState::Starting {
                    if peer.is_all_sources_received() {
                        peer.state = PeerState::Started;
                    } else {
                        peer.stop();
                        let at = peer.started_at.unwrap_or_else(Utc::now);
                        if let Some(room) = room.room.upgrade() {
                            room.do_send(PeerStopped { peer_id, at });
                        }
                    }
                };
            }
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
            for stat in this.stats.values_mut() {
                for peer in stat.peers.values_mut() {
                    peer.remove_outdated_sources(this.traffic_flowing_timeout);
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

/// Some [`FlowMetricSource`] notifies that it observes that
/// [`Peer`]s traffic is flowing.
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

    /// Source of this metric.
    source: FlowMetricSource,
}

impl Handler<TrafficFlows> for PeersTrafficWatcherImpl {
    type Result = ();

    /// Saves that provided [`FlowMetricSource`] reported that it observes
    /// [`Peer`] traffic flowing.
    ///
    /// If [`PeerStat`] is in [`PeerState::NotStarted`] state:
    /// 1. This stat is changed to [`PeerState::Starting`] state in which
    /// [`Peer`] init
    /// 2. [`PeerStarted`] message is sent to [`Room`].
    /// 3. [`PeersTrafficWatcherImpl::check_is_started`] is scheduled to run
    /// for this [`PeerStat`] in [`PeersTrafficWatcherImpl::peer_init_timeout`].
    ///
    /// If [`PeerStat`] is in [`PeerState::Starting`] state then provided
    /// [`FlowMetricSource`] is saved to list of received
    /// [`FlowMetricSource`]. This list will be checked in the
    /// [`PeersTrafficWatcherImpl::check_is_started`] function.
    ///
    /// If [`PeerStat`] is in [`PeerState::Started`] then last update time of
    /// the provided [`FlowMetricSource`] will be updated.
    ///
    /// If [`PeerStat`] is in [`PeerState::Stopped`] state then
    /// [`FlowMetricSource`] will be save and it'll check
    /// [`FlowMetricSource`]s will be received then [`PeerStat`] will be
    /// transferred into [`PeerState::Started`] with [`FlowMetricSource`]s from
    /// the [`PeerStat::Stopped`] state with [`Instant::now`] time.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    fn handle(
        &mut self,
        msg: TrafficFlows,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.peers.get_mut(&msg.peer_id) {
                peer.updated_at = Instant::now();
                peer.received_sources.insert(msg.source, Instant::now());
                match &mut peer.state {
                    PeerState::New => {
                        peer.state = PeerState::Starting;
                        peer.started_at = Some(Utc::now());

                        if let Some(room_addr) = room.room.upgrade() {
                            room_addr.do_send(PeerStarted(peer.peer_id));
                        }

                        ctx.run_later(
                            self.peer_init_timeout,
                            move |this, _| {
                                this.check_is_started(
                                    &msg.room_id,
                                    msg.peer_id,
                                );
                            },
                        );
                    }
                    PeerState::Stopped => {
                        if peer.is_all_sources_received() {
                            peer.state = PeerState::Started;
                            peer.started_at = Some(Utc::now());
                            if let Some(room_addr) = room.room.upgrade() {
                                room_addr.do_send(PeerStarted(peer.peer_id));
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
    }
}

/// Some [`FlowMetricSource`] notifies that it observes that
/// [`Peer`]s traffic stopped flowing.
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

    /// Sends [`PeerStopped`] into [`Room`] if [`PeerStat`] isn't in
    /// [`PeerState::Stopped`] state.
    ///
    /// Transfers [`PeerStat`] of the stopped [`Peer`] into
    /// [`PeerState::Stopped`].
    fn handle(
        &mut self,
        msg: TrafficStopped,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.peers.get_mut(&msg.peer_id) {
                if peer.state != PeerState::Stopped {
                    peer.stop();
                    if let Some(room_addr) = room.room.upgrade() {
                        let at = Utc::now()
                            - chrono::Duration::from_std(msg.at.elapsed())
                                .unwrap();
                        room_addr.do_send(PeerStopped {
                            peer_id: peer.peer_id,
                            at,
                        });
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
#[derive(Clone, Copy, Debug, PartialEq)]
enum PeerState {
    /// [`Peer`] was just added and have not received any traffic events.
    New,

    /// Some sources have reported that traffic is flowing, but not all of
    /// them.
    Starting,

    /// All of the sources have reported that traffic is flowing.
    Started,

    /// At least one of sources have reported that traffic has stopped.
    Stopped,
}

/// Current stats of [`Peer`].
///
/// Also this structure may be considered as subscription to this [`Peer`] state
/// updates.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug)]
struct PeerStat {
    /// [`PeerId`] of [`Peer`] which this [`PeerStat`] represents.
    ///
    /// [`Peer`]: crate::media::peer::Peer
    peer_id: PeerId,

    /// Current state of this [`PeerStat`].
    state: PeerState,

    /// List of [`FlowMetricSource`]s from which [`TrafficFlows`] should be
    /// received for validation that traffic is really going.
    tracked_sources: HashSet<FlowMetricSource>,

    /// [`DateTime`] when this [`PeerStat`] is started.
    ///
    /// If `None` then [`PeerStat`] not started.
    started_at: Option<DateTime<Utc>>,

    /// Time of last received [`PeerState`] proof.
    ///
    /// If [`PeerStat`] doesn't updates withing
    /// [`PeerTrafficWatcherImpl::traffic_flowing_timeout`] then this
    /// [`PeerStat`] will be considered as stopped and will be removed.
    updated_at: Instant,

    /// All [`FlowMetricSource`]s received at this moment with time at which
    /// they are received lastly.
    received_sources: HashMap<FlowMetricSource, Instant>,
}

impl PeerStat {
    /// Returns `true` if this [`PeerStat`] is considered valid.
    ///
    /// Checks that all [`FlowMetricSource`]s reported that traffic is flowing
    /// within `now() - traffic_flowing_timeout`.
    fn is_valid(&self, traffic_flowing_timeout: Duration) -> bool {
        if self.state == PeerState::Started {
            if !self.is_all_sources_received() {
                return false;
            }

            if self.updated_at.elapsed() > traffic_flowing_timeout {
                return false;
            }
        }

        true
    }

    /// Returns `false` if not all tracked [`FlowMetricSource`]s received by
    /// this [`PeerSpec`].
    ///
    /// Returns `true` if all tracked [`FlowMetricSource`]s received by this
    /// [`PeerSpec`].
    fn is_all_sources_received(&self) -> bool {
        for tracked_source in &self.tracked_sources {
            if !self.received_sources.contains_key(tracked_source) {
                return false;
            }
        }

        true
    }

    /// Removes all received [`FlowMetricSource`]s which received more than
    /// `traffic_flowing_timeout` ago.
    fn remove_outdated_sources(&mut self, traffic_flowing_timeout: Duration) {
        for src in &self.tracked_sources {
            if let Some(src_updated_at) = self.received_sources.get(src) {
                if src_updated_at.elapsed() > traffic_flowing_timeout {
                    self.received_sources.remove(src);
                }
            }
        }
    }

    /// Sets [`PeerStat`] state to the [`PeerState::Stopped`] and resets
    /// [`PeerStat::received_sources`].
    fn stop(&mut self) {
        self.state = PeerState::Stopped;
        self.received_sources = HashMap::new();
    }
}

/// Stores [`PeerStat`]s of [`Peer`]s for which stats updates [`Room`]
/// is watching.
///
/// [`Peer`]: crate::media::peer::Peer
#[derive(Debug)]
struct RoomStats {
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
        debug!(
            "Room [id = {}] was registered in the PeersTrafficWatcher.",
            msg.room_id
        );
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
/// [`PeersTrafficWatcherImpl`].
///
/// This message will just remove the subscription without emitting
/// [`TrafficStopped`] or [`PeerStopped`] messages.
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
        if self.stats.remove(&msg.0).is_some() {
            debug!(
                "Room [id = {}] was unregistered in the PeersTrafficWatcher.",
                msg.0
            );
        };
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
            debug!(
                "Peer [id = {}] from a Room [id = {}] was registered in the \
                 PeersTrafficWatcher with {:?} sources.",
                msg.peer_id, msg.room_id, msg.flow_metrics_sources
            );
            room.peers.insert(
                msg.peer_id,
                PeerStat {
                    peer_id: msg.peer_id,
                    state: PeerState::New,
                    tracked_sources: msg.flow_metrics_sources,
                    updated_at: Instant::now(),
                    started_at: None,
                    received_sources: HashMap::new(),
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
            let room_id = msg.room_id;
            for peer_id in msg.peers_ids {
                if room_stats.peers.remove(&peer_id).is_some() {
                    debug!(
                        "Peer [id = {}] from a Room [id = {}] was \
                         unregistered in the PeersTrafficWatcher.",
                        peer_id, room_id,
                    );
                };
            }
        }
    }
}

// TODO: unit tests
