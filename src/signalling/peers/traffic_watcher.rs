//! Provides [`PeerTrafficWatcher`] trait and its impl.
//!
//! [`PeerTrafficWatcher`] analyzes `Peer` traffic metrics and send messages
//! ([`PeerStarted`], [`PeerStopped`]) to [`Room`].
//!
//! Traffic metrics, consumed by [`PeerTrafficWatcher`] can originate from
//! different sources:
//! 1. [`FlowMetricSource::Peer`] - Stats received from member that owns target
//!    `Peer`.
//! 2. [`FlowMetricSource::PartnerPeer`] - Stats received from member,
//!    that owns `Peer`, connected to target `Peer`.
//! 3. [`FlowMetricSource::Coturn`] - Stats reported by Coturn TURN server, this
//! source is only being tracked if target `Peer` traffic is being relayed.
//!
//! At first you should register [`Room`] (`PeerTrafficWatcher.register_room()`)
//! and `Peer` (`PeerTrafficWatcher.register_peer()`). When first source will
//! report that traffic is flowing (`PeerTrafficWatcher.traffic_flows()`)
//! [`PeerStarted`] event will be sent to [`Room`].
//!
//! After that [`PeerTrafficWatcher`] will wait for other sources to report that
//! traffic is flowing for `init_timeout`, or [`PeerStopped`] event will be
//! sent to [`Room`].
//!
//! If some source will report that it observes traffic stopped flowing
//! (`PeerTrafficWatcher.traffic_stopped()`), then [`PeerStopped`] message will
//! be sent to [`Room`].

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
    time::{Duration, Instant},
};

use actix::{Actor, Addr, AsyncContext, Handler, MailboxError, Message};
use chrono::{DateTime, Utc};
use futures::future::LocalBoxFuture;
use medea_client_api_proto::PeerId;

use crate::{
    api::control::RoomId, conf, log::prelude::*, utils::instant_into_utc,
};

#[cfg_attr(test, mockall::automock)]
pub trait PeerTrafficWatcherSubscriber: Send + Debug {
    fn peer_started(&self, peer_id: PeerId);

    fn peer_stopped(&self, peer_id: PeerId, at: DateTime<Utc>);
}

#[cfg(test)]
impl Debug for MockPeerTrafficWatcherSubscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("MockPeerTrafficWatcherSubscriber").finish()
    }
}

/// Builds [`PeerTrafficWatcher`] backed by [`PeersTrafficWatcherImpl`] actor.
pub fn build_peers_traffic_watcher(
    conf: &conf::Media,
) -> Arc<dyn PeerTrafficWatcher> {
    Arc::new(PeersTrafficWatcherImpl::new(conf).start())
}

/// Message which indicates that `Peer` with provided [`PeerId`]
/// has started.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct PeerStarted(pub PeerId);

/// Message which indicates that `Peer` with provided [`PeerId`]
/// has stopped.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct PeerStopped {
    pub peer_id: PeerId,
    pub at: DateTime<Utc>,
}

/// Consumes `Peer` traffic metrics for further processing.
#[cfg_attr(test, mockall::automock)]
pub trait PeerTrafficWatcher: Debug + Send + Sync {
    /// Registers [`Room`] as `Peer`s state messages listener, preparing
    /// [`PeerTrafficWatcher`] for registering `Peer`s from this [`Room`].
    fn register_room(
        &self,
        room_id: RoomId,
        room: Box<dyn PeerTrafficWatcherSubscriber>,
    ) -> LocalBoxFuture<'static, Result<(), MailboxError>>;

    /// Unregisters [`Room`] as `Peer`s state messages listener.
    ///
    /// All `Peer` subscriptions related to this [`Room`] will be removed.
    fn unregister_room(&self, room_id: RoomId);

    /// Registers `Peer`, so that [`PeerTrafficWatcher`] will be able to
    /// process traffic flow events of this `Peer`.
    fn register_peer(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        should_watch_turn: bool,
    ) -> LocalBoxFuture<'static, Result<(), MailboxError>>;

    /// Unregisters `Peer`s, so that [`PeerTrafficWatcher`] will not be able
    /// to process traffic flow events of this `Peer` anymore.
    fn unregister_peers(&self, room_id: RoomId, peers_ids: Vec<PeerId>);

    /// Notifies [`PeerTrafficWatcher`] that some `Peer` traffic flowing.
    fn traffic_flows(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        source: FlowMetricSource,
    );

    /// Notifies [`PeerTrafficWatcher`] that some `Peer`s traffic flowing was
    /// stopped.
    fn traffic_stopped(&self, room_id: RoomId, peer_id: PeerId, at: Instant);
}

#[cfg(test)]
impl Debug for MockPeerTrafficWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("PeerTrafficWatcherMock").finish()
    }
}

/// Returns [`FlowMetricSources`], which will be used to emit `Peer` state
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

impl PeerTrafficWatcher for Addr<PeersTrafficWatcherImpl> {
    /// Sends [`RegisterRoom`] message to the [`PeersTrafficWatcherImpl`]
    /// returning send result.
    fn register_room(
        &self,
        room_id: RoomId,
        room: Box<dyn PeerTrafficWatcherSubscriber>,
    ) -> LocalBoxFuture<'static, Result<(), MailboxError>> {
        Box::pin(self.send(RegisterRoom { room_id, room }))
    }

    /// Sends [`UnregisterRoom`] message to [`PeersTrafficWatcherImpl`].
    fn unregister_room(&self, room_id: RoomId) {
        self.do_send(UnregisterRoom(room_id))
    }

    /// Sends [`RegisterPeer`] message to [`PeersTrafficWatcherImpl`] returning
    /// send result.
    fn register_peer(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        should_watch_turn: bool,
    ) -> LocalBoxFuture<'static, Result<(), MailboxError>> {
        Box::pin(self.send(RegisterPeer {
            room_id,
            peer_id,
            flow_metrics_sources: build_flow_sources(should_watch_turn),
        }))
    }

    /// Sends [`UnregisterPeers`] message to [`PeersTrafficWatcherImpl`].
    fn unregister_peers(&self, room_id: RoomId, peers_ids: Vec<PeerId>) {
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

/// Service which analyzes `Peer` traffic metrics and send messages
/// ([`PeerStarted`], [`PeerStopped`]) to [`Room`].
#[derive(Debug, Default)]
struct PeersTrafficWatcherImpl {
    /// All `Room`s which exists on the Medea server.
    stats: HashMap<RoomId, RoomStats>,

    /// Media source traffic report ttl. Media sources must continuously report
    /// that traffic is flowing, if some media source wont send new reports for
    /// this timeout, then it is considered that this source is not flowing any
    /// more.
    traffic_report_ttl: Duration,

    /// Duration after which [`PeersTrafficWatcherImpl`] will check that all
    /// tracked traffic sources have reported that traffic is flowing.
    init_timeout: Duration,
}

impl PeersTrafficWatcherImpl {
    /// Returns new [`PeersTrafficWatcherImpl`].
    pub fn new(conf: &conf::Media) -> Self {
        Self {
            stats: HashMap::new(),
            traffic_report_ttl: conf.max_lag,
            init_timeout: conf.init_timeout,
        }
    }

    /// Checks that all [`FlowMetricSource`] have reported that `Peer` traffic
    /// is flowing.
    ///
    /// If this check fails, then [`PeerStopped`] message is sent to [`Room`]
    /// with `at` field set at time, when first source reported that `Peer`
    /// traffic is flowing.
    ///
    /// Called for every `Peer` after
    /// `init_timeout` passed since first source reported that `Peer`
    /// traffic is flowing.
    fn check_is_started(&mut self, room_id: &RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(room_id) {
            if let Some(peer) = room.peers.get_mut(&peer_id) {
                if peer.state == PeerState::Starting {
                    if peer.is_flowing() {
                        peer.state = PeerState::Started;
                    } else {
                        peer.stop();
                        let at = peer.started_at.unwrap_or_else(Utc::now);
                        room.subscriber.peer_stopped(peer_id, at);
                    }
                };
            }
        }
    }
}

impl Actor for PeersTrafficWatcherImpl {
    type Context = actix::Context<Self>;

    /// Checks if [`PeerState::Started`] [`PeerStats`]s traffic is still
    /// flowing.
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_secs(1), |this, ctx| {
            for stat in this.stats.values_mut() {
                for peer in stat.peers.values_mut() {
                    if peer.state == PeerState::Started && !peer.is_flowing() {
                        peer.stop();
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
/// `Peer`s traffic is flowing.

#[derive(Debug, Message)]
#[rtype(result = "()")]
struct TrafficFlows {
    /// [`RoomId`] of [`Room`] where this `Peer` is stored.
    room_id: RoomId,

    /// [`PeerId`] of `Peer` which flows.
    peer_id: PeerId,

    /// Source of this metric.
    source: FlowMetricSource,
}

impl Handler<TrafficFlows> for PeersTrafficWatcherImpl {
    type Result = ();

    /// Saves that provided [`FlowMetricSource`] reported that it observes
    /// `Peer` traffic flowing.
    ///
    /// If [`PeerStat`] is in [`PeerState::NotStarted`] state:
    /// 1. This stat is changed to [`PeerState::Starting`] state in which
    /// `Peer` init
    /// 2. [`PeerStarted`] message is sent to [`Room`].
    /// 3. [`PeersTrafficWatcherImpl::check_is_started`] is scheduled to run
    /// for this [`PeerStat`] in [`PeersTrafficWatcherImpl::init_timeout`].
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
    fn handle(
        &mut self,
        msg: TrafficFlows,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.peers.get_mut(&msg.peer_id) {
                peer.received_sources.insert(msg.source, Instant::now());
                match &mut peer.state {
                    PeerState::New => {
                        peer.state = PeerState::Starting;
                        peer.started_at = Some(Utc::now());

                        room.subscriber.peer_started(peer.peer_id);

                        ctx.run_later(self.init_timeout, move |this, _| {
                            this.check_is_started(&msg.room_id, msg.peer_id);
                        });
                    }
                    PeerState::Stopped => {
                        if peer.is_flowing() {
                            peer.state = PeerState::Started;
                            peer.started_at = Some(Utc::now());
                            room.subscriber.peer_started(peer.peer_id);
                        }
                    }
                    _ => (),
                }
            }
        }
    }
}

/// Some [`FlowMetricSource`] notifies that it observes that
/// `Peer`s traffic stopped flowing.

#[derive(Debug, Message)]
#[rtype(result = "()")]
struct TrafficStopped {
    /// [`RoomId`] of [`Room`] where this `Peer` is stored.
    room_id: RoomId,

    /// [`PeerId`] of `Peer` which traffic was stopped.
    peer_id: PeerId,

    /// Time when proof of `Peer`s traffic stopping was gotten.
    at: Instant,
}

impl Handler<TrafficStopped> for PeersTrafficWatcherImpl {
    type Result = ();

    /// Sends [`PeerStopped`] into [`Room`] if [`PeerStat`] isn't in
    /// [`PeerState::Stopped`] state.
    ///
    /// Transfers [`PeerStat`] of the stopped `Peer` into
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
                    let at = instant_into_utc(msg.at);
                    room.subscriber.peer_stopped(peer.peer_id, at);
                }
            }
        }
    }
}

/// All sources of [`TrafficFlows`] message.
///
/// This is needed for checking that all metrics sources have the same opinion
/// about current `Peer`s traffic state.
///
/// [`PeerTrafficWatcher`] checks that all sources have the same opinion
/// after [`PeersTrafficWatcherImpl::init_timeout`] from first
/// [`TrafficFlows`] message received for some [`PeerStat`]. If at least one
/// [`FlowMetricSource`] doesn't sent [`TrafficFlows`] message, then `Peer`
/// will be considered as wrong and it will be stopped.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum FlowMetricSource {
    /// Metrics from the partner `Peer`.
    PartnerPeer,

    /// Metrics from the `Peer`.
    Peer,

    /// Metrics for this `Peer` from the Coturn TURN server.
    Coturn,
}

/// Current state of [`PeerStat`].
///
/// Transitions:
/// +-------+    +----------+    +-----------+     +-----------+
/// |  New  +--->+ Starting +--->+  Started  +<--->+  Stopped  |
/// +-------+    +----------+    +-----------+     +-----------+
#[derive(Clone, Copy, Debug, PartialEq)]
enum PeerState {
    /// `Peer` was just added and have not received any traffic events.
    New,

    /// Some sources have reported that traffic is flowing, but not all of
    /// them.
    Starting,

    /// All of the sources have reported that traffic is flowing.
    Started,

    /// At least one of sources have reported that traffic has stopped.
    Stopped,
}

/// Current stats of `Peer`.
///
/// Also this structure may be considered as subscription to this `Peer` state
/// updates.

#[derive(Debug)]
struct PeerStat {
    /// [`PeerId`] of `Peer` which this [`PeerStat`] represents.
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

    /// All [`FlowMetricSource`]s received at this moment with time at which
    /// they are received lastly.
    received_sources: HashMap<FlowMetricSource, Instant>,

    /// Media source traffic report ttl. Media sources must continuously report
    /// that traffic is flowing, if some media source wont send new reports for
    /// this timeout, then it is considered that this source is not flowing any
    /// more.
    traffic_flowing_timeout: Duration,
}

impl PeerStat {
    /// Returns `true` if this [`PeerStat`] is considered valid.
    ///
    /// Checks that all required [`FlowMetricSource`]s reported that traffic is
    /// flowing within `now() - traffic_flowing_timeout`.
    fn is_flowing(&self) -> bool {
        for tracked_source in &self.tracked_sources {
            if let Some(at) = self.received_sources.get(tracked_source) {
                if at.elapsed() > self.traffic_flowing_timeout {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    /// Sets [`PeerStat`] state to the [`PeerState::Stopped`] and resets
    /// [`PeerStat::received_sources`].
    fn stop(&mut self) {
        self.state = PeerState::Stopped;
        self.received_sources.clear();
    }
}

/// Stores [`PeerStat`]s of `Peer`s for which stats updates [`Room`]
/// is watching.

#[derive(Debug)]
struct RoomStats {
    /// [`RoomId`] of all [`PeerStat`] which stored here.
    room_id: RoomId,

    /// [`Addr`] of [`Room`] which is watching for this [`PeerStat`]s.
    subscriber: Box<dyn PeerTrafficWatcherSubscriber>,

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
    room: Box<dyn PeerTrafficWatcherSubscriber>,
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
                subscriber: msg.room,
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
                    started_at: None,
                    received_sources: HashMap::new(),
                    traffic_flowing_timeout: self.traffic_report_ttl,
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
    peers_ids: Vec<PeerId>,
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

#[cfg(test)]
mod tests {
    use futures::{channel::mpsc, stream::LocalBoxStream, StreamExt};

    use super::*;

    use crate::utils::test::future_with_timeout;

    /// Helper for the all [`traffic_watcher`] unit tests.
    struct Helper {
        /// Stream which will receive all sent by [`PeersTrafficWatcher`]
        /// [`PeerStopped`] messages.
        peer_stopped_rx: LocalBoxStream<'static, PeerStopped>,

        /// Stream which will receive all sent by [`PeersTrafficWatcher`]
        /// [`PeerStarted`] messages.
        peer_started_rx: LocalBoxStream<'static, PeerStarted>,

        /// [`PeerTrafficWatcherImpl`] [`Actor`].
        traffic_watcher: Addr<PeersTrafficWatcherImpl>,
    }

    impl Helper {
        /// Returns new [`Helper`] with empty [`PeersTrafficWatcher`].
        pub async fn new(cfg: &conf::Media) -> Self {
            let watcher = PeersTrafficWatcherImpl::new(cfg).start();
            let mut subscriber = MockPeerTrafficWatcherSubscriber::new();
            let (peer_stopped_tx, peer_stopped_rx) = mpsc::unbounded();
            let (peer_started_tx, peer_started_rx) = mpsc::unbounded();
            subscriber
                .expect_peer_stopped()
                .returning(move |peer_id, at| {
                    peer_stopped_tx
                        .unbounded_send(PeerStopped { peer_id, at })
                        .unwrap();
                });
            subscriber.expect_peer_started().returning(move |peer_id| {
                peer_started_tx
                    .unbounded_send(PeerStarted(peer_id))
                    .unwrap();
            });
            watcher
                .register_room(Self::room_id(), Box::new(subscriber))
                .await
                .unwrap();

            Self {
                traffic_watcher: watcher,
                peer_started_rx: Box::pin(peer_started_rx),
                peer_stopped_rx: Box::pin(peer_stopped_rx),
            }
        }

        /// Returns [`RoomId`] used for the [`traffic_watcher`] unit tests.
        fn room_id() -> RoomId {
            "test-room".to_string().into()
        }

        /// Returns [`Addr`] to the underlying [`PeersTrafficWatcherImpl`].
        pub fn watcher(&self) -> Addr<PeersTrafficWatcherImpl> {
            self.traffic_watcher.clone()
        }

        /// Waits for the [`PeerStopped`] event which
        /// [`PeersTrafficWatcherImpl`] sends to the [`Room`].
        pub async fn next_peer_stopped(&mut self) -> PeerStopped {
            self.peer_stopped_rx.next().await.unwrap()
        }

        /// Waits for the [`PeerStarted`] event which
        /// [`PeersTrafficWatcherImpl`] sends to the [`Room`].
        pub async fn next_peer_started(&mut self) -> PeerStarted {
            self.peer_started_rx.next().await.unwrap()
        }
    }

    /// Checks that [`PeerTrafficWatcherImpl`] normally sends [`PeerStarted`]
    /// and [`PeerStopped`] messages to the [`Room`] on normal traffic
    /// flowing cycle.
    #[actix_rt::test]
    async fn two_sources_works() {
        let mut helper = Helper::new(&conf::Media {
            init_timeout: Duration::from_millis(150),
            max_lag: Duration::from_millis(300),
        })
        .await;
        helper
            .watcher()
            .register_peer(Helper::room_id(), PeerId(1), false)
            .await
            .unwrap();
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::Peer,
        );
        assert_eq!(helper.next_peer_started().await.0, PeerId(1));
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::PartnerPeer,
        );
        future_with_timeout(
            helper.next_peer_stopped(),
            Duration::from_millis(150),
        )
        .await
        .unwrap_err();
    }

    /// Checks that in [`PeerStopped`] message correct stop time will be
    /// provided.
    #[actix_rt::test]
    async fn at_in_stop_on_start_checking_is_valid() {
        let mut helper = Helper::new(&conf::Media {
            init_timeout: Duration::from_millis(100),
            ..Default::default()
        })
        .await;
        helper
            .watcher()
            .register_peer(Helper::room_id(), PeerId(1), false)
            .await
            .unwrap();
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::Peer,
        );
        assert_eq!(helper.next_peer_started().await.0, PeerId(1));
        let start_time = Utc::now();
        assert_eq!(
            helper.next_peer_stopped().await.at.timestamp() / 10,
            start_time.timestamp() / 10
        );
    }

    /// Checks that [`TrafficStopped`] will be sent if no [`TrafficFlows`] will
    /// be received within `max_lag` timeout.
    #[actix_rt::test]
    async fn stop_on_max_lag() {
        let mut helper = Helper::new(&conf::Media {
            init_timeout: Duration::from_millis(30),
            max_lag: Duration::from_millis(30),
            ..Default::default()
        })
        .await;
        helper
            .watcher()
            .register_peer(Helper::room_id(), PeerId(1), false)
            .await
            .unwrap();
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::Peer,
        );
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::PartnerPeer,
        );

        future_with_timeout(
            helper.next_peer_stopped(),
            Duration::from_secs(1) + Duration::from_millis(10),
        )
        .await
        .unwrap();
    }
}
