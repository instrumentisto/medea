//! Provides [`PeerTrafficWatcher`] trait and its impl.
//!
//! [`PeerTrafficWatcher`] analyzes `Peer` traffic metrics and notifies
//! [`PeerConnectionStateEventsHandler`] about traffic flowing changes.
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
//! [`PeerConnectionStateEventsHandler::peer_started`] will be called.
//!
//! After that [`PeerTrafficWatcher`] will wait for other sources to report that
//! traffic is flowing for `init_timeout`, or
//! [`PeerConnectionStateEventsHandler::peer_stopped`] will be called.
//!
//! If some source will report that it observes traffic stopped flowing
//! (`PeerTrafficWatcher.traffic_stopped()`), then
//! [`PeerConnectionStateEventsHandler::peer_stopped`] will be called.
//!
//! [`Room`]: crate::signalling::room::Room

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
    time::{Duration, Instant},
};

use actix::{
    Actor, Addr, AsyncContext, Handler, MailboxError, Message, SpawnHandle,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use medea_client_api_proto::{PeerId, RoomId};

use crate::{conf, log::prelude::*, utils::instant_into_utc};

/// Subscriber of `Peer` traffic flowing changes.
#[cfg_attr(test, mockall::automock)]
pub trait PeerConnectionStateEventsHandler: Send + Debug {
    /// [`PeerTrafficWatcher`] believes that traffic was started.
    fn peer_started(&self, peer_id: PeerId);

    /// [`PeerTrafficWatcher`] believes that traffic was stopped.
    fn peer_stopped(&self, peer_id: PeerId, at: DateTime<Utc>);
}

#[cfg(test)]
impl_debug_by_struct_name!(MockPeerConnectionStateEventsHandler);

/// Builds [`PeerTrafficWatcher`].
#[cfg(test)]
pub fn build_peers_traffic_watcher(
    conf: &conf::Media,
) -> Arc<dyn PeerTrafficWatcher> {
    Arc::new(PeersTrafficWatcherImpl::new(conf).start())
}

// TODO: Returns dummy implementation cause this component is currently
//       disabled.
//       Will be enabled in https://github.com/instrumentisto/medea/pull/91
/// Builds [`PeerTrafficWatcher`].
#[cfg(not(test))]
#[must_use]
pub fn build_peers_traffic_watcher(
    _: &conf::Media,
) -> Arc<dyn PeerTrafficWatcher> {
    #[derive(Debug)]
    struct DummyPeerTrafficWatcher;

    #[async_trait(?Send)]
    impl PeerTrafficWatcher for DummyPeerTrafficWatcher {
        async fn register_room(
            &self,
            _: RoomId,
            _: Box<dyn PeerConnectionStateEventsHandler>,
        ) -> Result<(), MailboxError> {
            Ok(())
        }

        fn unregister_room(&self, _: RoomId) {}

        async fn register_peer(
            &self,
            _: RoomId,
            _: PeerId,
            _: bool,
        ) -> Result<(), MailboxError> {
            Ok(())
        }

        fn unregister_peers(&self, _: RoomId, _: Vec<PeerId>) {}

        fn traffic_flows(&self, _: RoomId, _: PeerId, _: FlowMetricSource) {}

        fn traffic_stopped(&self, _: RoomId, _: PeerId, _: Instant) {}
    }
    Arc::new(DummyPeerTrafficWatcher)
}

/// Consumes `Peer` traffic metrics for further processing.
#[async_trait(?Send)]
#[cfg_attr(test, mockall::automock)]
pub trait PeerTrafficWatcher: Debug + Send + Sync {
    /// Registers provided [`PeerConnectionStateEventsHandler`] as `Peer`s state
    /// messages listener, preparing [`PeerTrafficWatcher`] for registering
    /// `Peer`s from this [`PeerConnectionStateEventsHandler`].
    async fn register_room(
        &self,
        room_id: RoomId,
        handler: Box<dyn PeerConnectionStateEventsHandler>,
    ) -> Result<(), MailboxError>;

    /// Unregisters [`Room`] as `Peer`s state messages listener.
    ///
    /// All `Peer` subscriptions related to this [`Room`] will be removed.
    ///
    /// [`Room`]: crate::signalling::room::Room
    fn unregister_room(&self, room_id: RoomId);

    /// Registers `Peer`, so that [`PeerTrafficWatcher`] will be able to
    /// process traffic flow events of this `Peer`.
    async fn register_peer(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        should_watch_turn: bool,
    ) -> Result<(), MailboxError>;

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
impl_debug_by_struct_name!(MockPeerTrafficWatcher);

/// Returns [`FlowMetricSource`]s, which will be used to emit `Peer` state
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

#[async_trait(?Send)]
impl PeerTrafficWatcher for Addr<PeersTrafficWatcherImpl> {
    async fn register_room(
        &self,
        room_id: RoomId,
        handler: Box<dyn PeerConnectionStateEventsHandler>,
    ) -> Result<(), MailboxError> {
        self.send(RegisterRoom { room_id, handler }).await
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

    fn unregister_peers(&self, room_id: RoomId, peers_ids: Vec<PeerId>) {
        self.do_send(UnregisterPeers { room_id, peers_ids })
    }

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

    fn traffic_stopped(&self, room_id: RoomId, peer_id: PeerId, at: Instant) {
        debug!("TrafficStopped: in {}/{}", room_id, peer_id);
        self.do_send(TrafficStopped {
            room_id,
            peer_id,
            at,
        })
    }
}

/// Service which analyzes `Peer` traffic metrics and notifies about traffic
/// flowing changes [`PeerConnectionStateEventsHandler`]s.
#[derive(Debug, Default)]
pub struct PeersTrafficWatcherImpl {
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
    /// is flowing for `Peer` in `PeerState::Starting` state.
    ///
    /// If this check fails, then
    /// [`PeerConnectionStateEventsHandler::peer_stopped`] will be called with a
    /// time, when first source reported that `Peer` traffic is flowing.
    ///
    /// If check succeeds then `Peer` is transitioned to `PeerState::Started`
    /// state.
    ///
    /// Called for every `Peer` after `init_timeout` passed since first source
    /// reported that `Peer` traffic is flowing.
    fn check_is_started(&mut self, room_id: &RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(room_id) {
            if let Some(peer) = room.peers.get_mut(&peer_id) {
                if peer.state == PeerState::Starting {
                    if peer.is_flowing() {
                        peer.state = PeerState::Started;
                    } else {
                        peer.stop();
                        let at = peer.started_at.unwrap_or_else(Utc::now);
                        room.handler.peer_stopped(peer_id, at);
                    }
                };
            }
        }
    }
}

impl Actor for PeersTrafficWatcherImpl {
    type Context = actix::Context<Self>;

    /// Checks if [`PeerState::Started`] [`PeerStat`]s traffic is still
    /// flowing.
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_secs(1), |this, _| {
            for room in this.stats.values_mut() {
                for peer in room.peers.values_mut() {
                    if peer.state == PeerState::Started && !peer.is_flowing() {
                        peer.stop();
                        room.handler.peer_stopped(
                            peer.peer_id,
                            instant_into_utc(Instant::now()),
                        );
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
    ///
    /// [`Room`]: crate::signalling::room::Room
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
    /// If [`PeerStat`] is in [`PeerState::Stopped`] state:
    /// 1. This stat is changed to [`PeerState::Starting`] state in which
    /// `Peer` init
    /// 2. [`PeerConnectionStateEventsHandler::peer_started`] is called.
    /// 3. [`PeersTrafficWatcherImpl::check_is_started`] is scheduled to run
    ///    for this [`PeerStat`] in [`PeersTrafficWatcherImpl::init_timeout`].
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
    /// the [`PeerState::Stopped`] state with [`Instant::now`] time.
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

                        room.handler.peer_started(peer.peer_id);

                        let init_check_task_handle =
                            ctx.run_later(self.init_timeout, move |this, _| {
                                this.check_is_started(
                                    &msg.room_id,
                                    msg.peer_id,
                                );
                            });
                        peer.init_task_handler.replace(init_check_task_handle);
                    }
                    PeerState::Starting => {
                        if peer.state == PeerState::Starting
                            && peer.is_flowing()
                        {
                            peer.state = PeerState::Started;
                            peer.init_task_handler.take();
                        };
                    }
                    PeerState::Stopped => {
                        if peer.is_flowing() {
                            peer.state = PeerState::Started;
                            peer.started_at = Some(Utc::now());
                            room.handler.peer_started(peer.peer_id);
                        }
                    }
                    PeerState::Started => (),
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
    ///
    /// [`Room`]: crate::signalling::room::Room
    room_id: RoomId,

    /// [`PeerId`] of `Peer` which traffic was stopped.
    peer_id: PeerId,

    /// Time when proof of `Peer`s traffic stopping was gotten.
    at: Instant,
}

impl Handler<TrafficStopped> for PeersTrafficWatcherImpl {
    type Result = ();

    /// Calls [`PeerConnectionStateEventsHandler::peer_stopped`] if [`PeerStat`]
    /// isn't in [`PeerState::Stopped`] state.
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
                    room.handler
                        .peer_stopped(peer.peer_id, instant_into_utc(msg.at));
                }
            }
        }
    }
}

/// All possible sources of traffic flows signal.
///
/// It's considered that traffic is flowing if all listed sources confirm that
/// it does.
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

    /// `SpawnHandle` to `Peer` init task (`check_is_started`)
    init_task_handler: Option<SpawnHandle>,

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
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Debug)]
struct RoomStats {
    /// [`RoomId`] of all [`PeerStat`] which stored here.
    room_id: RoomId,

    /// Handler of the [`PeerStat`] events.
    handler: Box<dyn PeerConnectionStateEventsHandler>,

    /// [`PeerStat`] for which [`Room`] is watching.
    ///
    /// [`Room`]: crate::signalling::room::Room
    peers: HashMap<PeerId, PeerStat>,
}

/// Registers new [`Room`] as [`PeerStat`]s listener.
///
/// This message will only add provided [`Room`] to the list. For real
/// subscription to a [`PeerStat`] [`RegisterPeer`] message should be used.
///
/// [`RegisterRoom`] will be called in [`RoomService`] for every [`Room`] when
/// it created.
///
/// [`Room`]: crate::signalling::room::Room
/// [`RoomService`]: crate::signalling::room_service::RoomService
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct RegisterRoom {
    /// [`RoomId`] of [`Room`] which requested to register in the
    /// [`PeersTrafficWatcherImpl`].
    ///
    /// [`Room`]: crate::signalling::room::Room
    room_id: RoomId,

    /// Handler of the [`PeerStat`] events.
    handler: Box<dyn PeerConnectionStateEventsHandler>,
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
                handler: msg.handler,
                peers: HashMap::new(),
            },
        );
    }
}

/// Unregisters [`Room`] with provided [`RoomId`] from the
/// [`PeersTrafficWatcherImpl`].
///
/// This message will just remove the subscription without emitting
/// any stop events.
///
/// [`Room`]: crate::signalling::room::Room
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
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct RegisterPeer {
    /// [`RoomId`] of [`Room`] which subscribes on [`PeerStat`]'s [`PeerState`]
    /// changes.
    ///
    /// [`Room`]: crate::signalling::room::Room
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
            if let Some(peer) = room.peers.get_mut(&msg.peer_id) {
                peer.tracked_sources.extend(msg.flow_metrics_sources);
            } else {
                debug!(
                    "Peer [id = {}] from a Room [id = {}] was registered in \
                     the PeersTrafficWatcher with {:?} sources.",
                    msg.peer_id, msg.room_id, msg.flow_metrics_sources
                );
                room.peers.insert(
                    msg.peer_id,
                    PeerStat {
                        peer_id: msg.peer_id,
                        state: PeerState::New,
                        init_task_handler: None,
                        tracked_sources: msg.flow_metrics_sources,
                        started_at: None,
                        received_sources: HashMap::new(),
                        traffic_flowing_timeout: self.traffic_report_ttl,
                    },
                );
            }
        }
    }
}

/// Unregisters [`Room`] with provided [`RoomId`] from [`PeerStat`] with
/// provided [`PeerId`] updates receiving.
///
/// [`Room`]: crate::signalling::room::Room
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct UnregisterPeers {
    /// [`RoomId`] of [`Room`] which unregisters from [`PeerStat`]'s
    /// [`PeerState`] changes.
    ///
    /// [`Room`]: crate::signalling::room::Room
    room_id: RoomId,

    /// [`PeerId`] of [`PeerStat`] from which unregistration is requested.
    ///
    /// [`Room`]: crate::signalling::room::Room
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

    use tokio::time::timeout;

    use super::*;

    /// Helper for the all [`traffic_watcher`] unit tests.
    struct Helper {
        /// Stream which will receive all
        /// [`PeerConnectionStateEventsHandler::peer_stopped`] calls.
        peer_stopped_rx: LocalBoxStream<'static, (PeerId, DateTime<Utc>)>,

        /// Stream which will receive all
        /// [`PeerConnectionStateEventsHandler::peer_started`] calls.
        peer_started_rx: LocalBoxStream<'static, PeerId>,

        /// [`PeerTrafficWatcherImpl`] [`Actor`].
        traffic_watcher: Addr<PeersTrafficWatcherImpl>,
    }

    impl Helper {
        /// Returns new [`Helper`] with empty [`PeersTrafficWatcher`].
        pub async fn new(cfg: &conf::Media) -> Self {
            let watcher = PeersTrafficWatcherImpl::new(cfg).start();
            let mut handler = MockPeerConnectionStateEventsHandler::new();
            let (peer_stopped_tx, peer_stopped_rx) = mpsc::unbounded();
            let (peer_started_tx, peer_started_rx) = mpsc::unbounded();
            handler.expect_peer_stopped().returning(move |peer_id, at| {
                peer_stopped_tx.unbounded_send((peer_id, at)).unwrap();
            });
            handler.expect_peer_started().returning(move |peer_id| {
                peer_started_tx.unbounded_send(peer_id).unwrap();
            });
            watcher
                .register_room(Self::room_id(), Box::new(handler))
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
            RoomId::from("test-room")
        }

        /// Returns [`Addr`] to the underlying [`PeersTrafficWatcherImpl`].
        pub fn watcher(&self) -> Addr<PeersTrafficWatcherImpl> {
            self.traffic_watcher.clone()
        }

        /// Waits for the [`PeerConnectionStateEventsHandler::peer_stopped`]
        /// call.
        pub async fn next_peer_stopped(&mut self) -> (PeerId, DateTime<Utc>) {
            self.peer_stopped_rx.next().await.unwrap()
        }

        /// Waits for the [`PeerConnectionStateEventsHandler::peer_started`]
        /// call.
        pub async fn next_peer_started(&mut self) -> PeerId {
            self.peer_started_rx.next().await.unwrap()
        }
    }

    /// Checks that [`PeerTrafficWatcher`] provides correct stop time into
    /// [`PeerConnectionStateEventsHandler::peer_stopped`] function.
    #[actix_rt::test]
    async fn correct_stopped_at_when_init_timeout_stop() {
        let mut helper = Helper::new(&conf::Media {
            init_timeout: Duration::from_millis(100),
            max_lag: Duration::from_secs(999),
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
        assert_eq!(helper.next_peer_started().await, PeerId(1));
        let start_time = Utc::now();
        let (_, at) = helper.next_peer_stopped().await;
        assert_eq!(at.timestamp() / 10, start_time.timestamp() / 10);
    }

    /// Checks that [`PeerConnectionStateEventsHandler::peer_stopped`] will be
    /// called if no [`TrafficFlows`] will be received within `max_lag`
    /// timeout.
    async fn stop_on_max_lag_helper() -> Helper {
        let mut helper = Helper::new(&conf::Media {
            init_timeout: Duration::from_secs(999),
            max_lag: Duration::from_millis(50),
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
        timeout(Duration::from_millis(30), helper.next_peer_started())
            .await
            .unwrap();
        timeout(Duration::from_millis(1100), helper.next_peer_stopped())
            .await
            .unwrap();
        helper
    }

    #[actix_rt::test]
    async fn stop_on_max_lag() {
        stop_on_max_lag_helper().await;
    }

    /// Checks correct `Peer` start after it was stopped cause max lag timeout
    /// was exceeded.
    #[actix_rt::test]
    async fn start_after_stop_on_max_lag() {
        let mut helper = stop_on_max_lag_helper().await;
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::Peer,
        );
        timeout(Duration::from_millis(30), helper.next_peer_started())
            .await
            .unwrap_err();
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::PartnerPeer,
        );
        timeout(Duration::from_millis(30), helper.next_peer_started())
            .await
            .unwrap();
    }

    /// Helper for `init_timeout` tests.
    /// 1. Creates `PeersTrafficWatcherImpl` with `init_timeout = 30ms`, and
    /// `max_lag = 999s`.
    /// 2. Registers `Peer` with provided `should_watch_turn`
    /// 3. Invokes `traffic_flows` for each provided
    /// `traffic_flows_invocations`.
    /// 4. Expects `peer_started` within `50ms` if `should_start = true`.
    /// 5. Expects `peer_stopped` within `50ms` if `should_stop = true`.
    async fn init_timeout_tests_helper(
        should_watch_turn: bool,
        traffic_flows_invocations: &[FlowMetricSource],
        should_start: bool,
        should_stop: bool,
    ) -> Helper {
        let mut helper = Helper::new(&conf::Media {
            init_timeout: Duration::from_millis(30),
            max_lag: Duration::from_secs(999),
        })
        .await;
        helper
            .watcher()
            .register_peer(Helper::room_id(), PeerId(1), should_watch_turn)
            .await
            .unwrap();
        for source in traffic_flows_invocations {
            helper.watcher().traffic_flows(
                Helper::room_id(),
                PeerId(1),
                *source,
            );
        }

        let peer_started =
            timeout(Duration::from_millis(50), helper.next_peer_started())
                .await;
        if should_start {
            peer_started.unwrap();
        } else {
            peer_started.unwrap_err();
        }

        let peer_stopped =
            timeout(Duration::from_millis(50), helper.next_peer_stopped())
                .await;
        if should_stop {
            peer_stopped.unwrap();
        } else {
            peer_stopped.unwrap_err();
        };

        helper
    }

    /// Pass different combinations of `traffic_flows` to concrete peer and see
    /// if `init_timeout` triggers.
    #[actix_rt::test]
    async fn init_timeout_tests() {
        use FlowMetricSource::{Coturn, PartnerPeer, Peer};

        init_timeout_tests_helper(false, &[], false, false).await;
        init_timeout_tests_helper(false, &[Peer], true, true).await;
        init_timeout_tests_helper(false, &[Peer, Peer], true, true).await;
        init_timeout_tests_helper(false, &[Peer, Coturn], true, true).await;
        init_timeout_tests_helper(true, &[Peer, PartnerPeer], true, true).await;

        init_timeout_tests_helper(false, &[Peer, PartnerPeer], true, false)
            .await;
        init_timeout_tests_helper(
            true,
            &[Peer, PartnerPeer, Coturn],
            true,
            false,
        )
        .await;
    }

    /// Checks correct `Peer` start after it was stopped cause init timeout
    /// was exceeded.
    #[actix_rt::test]
    async fn start_after_init_timeout_stop() {
        let mut helper = init_timeout_tests_helper(
            false,
            &[FlowMetricSource::Peer],
            true,
            true,
        )
        .await;
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::Peer,
        );
        timeout(Duration::from_millis(30), helper.next_peer_started())
            .await
            .unwrap_err();
        helper.watcher().traffic_flows(
            Helper::room_id(),
            PeerId(1),
            FlowMetricSource::PartnerPeer,
        );
        timeout(Duration::from_millis(30), helper.next_peer_started())
            .await
            .unwrap();
    }

    #[actix_rt::test]
    async fn peer_stop_when_traffic_stop() {
        {
            // `traffic_stopped` on started `Peer`
            let mut helper = init_timeout_tests_helper(
                false,
                &[FlowMetricSource::Peer, FlowMetricSource::PartnerPeer],
                true,
                false,
            )
            .await;
            helper.watcher().traffic_stopped(
                Helper::room_id(),
                PeerId(1),
                Instant::now(),
            );
            timeout(Duration::from_millis(10), helper.next_peer_stopped())
                .await
                .unwrap();
        }
        {
            // `traffic_stopped` on starting `Peer`
            let mut helper = Helper::new(&conf::Media {
                init_timeout: Duration::from_secs(999),
                max_lag: Duration::from_secs(999),
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
            timeout(Duration::from_millis(10), helper.next_peer_started())
                .await
                .unwrap();
            helper.watcher().traffic_stopped(
                Helper::room_id(),
                PeerId(1),
                Instant::now(),
            );
            timeout(Duration::from_millis(10), helper.next_peer_stopped())
                .await
                .unwrap();
        }
        {
            // `traffic_stopped` on stopped `Peer`
            let mut helper = init_timeout_tests_helper(
                false,
                &[FlowMetricSource::Peer],
                true,
                true,
            )
            .await;
            helper.watcher().traffic_stopped(
                Helper::room_id(),
                PeerId(1),
                Instant::now(),
            );
            timeout(Duration::from_millis(10), helper.next_peer_stopped())
                .await
                .unwrap_err();
        }
    }
}
