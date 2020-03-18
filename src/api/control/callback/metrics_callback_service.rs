//! Service which is responsible to `Peer` metrics based Control API callbacks.
//!
//! `Peer` metrics will be collected from the many sources of metrics. All this
//! metrics will be collected and based on them, [`MetricsCallbackService`] will
//! consider which callback should be sent (or not sent).
//!
//! # List of `Peer` metrics based Control API callbacks:
//!
//! 1. `WebRtcPublishEndpoint::on_start`;
//! 2. `WebRtcPublishEndpoint::on_stop`;
//! 3. `WebRtcPlayEndpoint::on_start`;
//! 4. `WebRtcPlayEndpoint::on_stop`.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use actix::{Actor, Addr, AsyncContext, Handler, Message};
use medea_client_api_proto::PeerId;
use variant_count::VariantCount;

use crate::{
    api::control::RoomId,
    signalling::{
        room::{PeerSpecContradiction, PeerStarted, PeerStopped},
        Room,
    },
};

/// Service which responsible for the `Peer` metrics based Control API
/// callbacks.
#[derive(Debug, Default)]
pub struct MetricsCallbacksService {
    /// All `Room` which exists on the Medea server.
    stats: HashMap<RoomId, RoomStats>,
}

impl MetricsCallbacksService {
    /// Returns new [`MetricsCallbacksService`].
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    /// Unsubscribes [`MetricsCallbackService`] from watching a `Peer` with
    /// provided [`PeerId`].
    ///
    /// Removes provided [`PeerId`] from [`RoomStat`] with provided [`RoomId`].
    pub fn unsubscribe_from_peer(&mut self, room_id: &RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(room_id) {
            room.peers.remove(&peer_id);
        }
    }

    /// Unsubscribes [`MetricsCallbackService`] from a `Peer` with fatal error
    /// and notifies [`Room`] about fatal error in [`PeerStat`].
    fn fatal_peer_error(&mut self, room_id: &RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(&room_id) {
            room.peers.remove(&peer_id);
            room.room.do_send(PeerSpecContradiction { peer_id });
        }
    }

    /// Checks that all metrics sources considered that `Peer` with provided
    /// [`PeerId`] is started.
    ///
    /// This function will be called on every [`PeerStat`] after `10sec` from
    /// first [`PeerStat`]'s [`TrafficFlows`] message.
    ///
    /// If this check fails then [`MetricsCallbackService::fatal_peer_error`]
    /// will be called for this [`PeerStat`].
    fn check_on_start(&mut self, room_id: &RoomId, peer_id: PeerId) {
        let peer = self
            .stats
            .get_mut(room_id)
            .and_then(|room| room.peers.get_mut(&peer_id));

        if let Some(peer) = peer {
            if let PeerState::Started(srcs) = &peer.state {
                let is_not_all_sources_sent_start =
                    srcs.len() < FlowMetricSource::VARIANT_COUNT;
                if is_not_all_sources_sent_start {
                    self.fatal_peer_error(room_id, peer_id);
                }
            }
        }
    }
}

impl Actor for MetricsCallbacksService {
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
                                timestamp: Instant::now(),
                            });
                        }
                    }
                }
            }
        });
    }
}

/// Some [`FlowMetricSource`] notifies [`MetricsCallbacksService`] that `Peer`
/// with provided [`PeerId`] is normally flows.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TrafficFlows {
    /// [`RoomId`] of [`Room`] where this `Peer` is stored.
    pub room_id: RoomId,

    /// [`PeerId`] of `Peer` which flows.
    pub peer_id: PeerId,

    /// Time when proof of `Peer`'s traffic flowing was gotten.
    pub timestamp: Instant,

    /// Source of this metric.
    pub source: FlowMetricSource,
}

impl Handler<TrafficFlows> for MetricsCallbacksService {
    type Result = ();

    fn handle(
        &mut self,
        msg: TrafficFlows,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.peers.get_mut(&msg.peer_id) {
                peer.last_update = msg.timestamp;
                match &mut peer.state {
                    PeerState::Started(sources) => {
                        sources.insert(msg.source);
                    }
                    PeerState::Stopped => {
                        let mut srcs = HashSet::new();
                        srcs.insert(msg.source);
                        peer.state = PeerState::Started(srcs);

                        ctx.run_later(
                            Duration::from_secs(15),
                            move |this, _| {
                                this.check_on_start(&msg.room_id, msg.peer_id);
                            },
                        );

                        room.room.do_send(PeerStarted(peer.peer_id));
                    }
                }
            }
        }
    }
}

/// Some [`StoppedMetricSource`] notifies [`MetricsCallbacksService`] that
/// traffic flowing of `Peer` with provided [`PeerId`] was stopped.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TrafficStopped {
    /// [`RoomId`] of [`Room`] where this `Peer` is stored.
    pub room_id: RoomId,

    /// [`PeerId`] of `Peer` which traffic was stopped.
    pub peer_id: PeerId,

    /// Time when proof of `Peer`'s traffic stopping was gotten.
    pub timestamp: Instant,

    /// Source of this metric.
    pub source: StoppedMetricSource,
}

impl Handler<TrafficStopped> for MetricsCallbacksService {
    type Result = ();

    fn handle(
        &mut self,
        msg: TrafficStopped,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.peers.remove(&msg.peer_id) {
                println!(
                    "Peer #{} stopped basic on {:?}.",
                    msg.peer_id, msg.source
                );
                room.room.do_send(PeerStopped(peer.peer_id));
            }
        }
        self.unsubscribe_from_peer(&msg.room_id, msg.peer_id);
    }
}

/// All sources of [`TrafficFlows`] message.
///
/// This is needed for checking that all metrics sources have the same opinion
/// about current `PeerConnection`'s traffic state.
///
/// [`MetricsCallbackService`] checks that all sources have the same opinion
/// after `10secs` from first [`TrafficFlows`] message received for some
/// [`PeerStat`]. If at least one [`FlowMetricSource`] doesn't sent
/// [`TrafficFlows`] message, then `PeerConnection` will be considered as wrong
/// and it will be stopped.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, VariantCount)]
pub enum FlowMetricSource {
    /// Metrics from the partner `PeerConnection`.
    PartnerPeerTraffic,

    /// Metrics from the `PeerConnection`.
    PeerTraffic,

    /// Metrics for this `PeerConnection` from the Coturn TURN server.
    Coturn,
}

/// All sources of [`TrafficStopped`] message.
// TODO: maybe this is not needed???
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum StoppedMetricSource {
    PartnerPeerRemoved,
    PeerTraffic,
    Coturn,
    Timeout,
    PeerRemoved,
}

/// Current state of [`PeerStat`].
///
/// If [`PeerStat`] goes into [`PeerState::Started`] then all
/// [`FlowMetricSource`]s should notify [`MetricsCallbackService`] about it.
#[derive(Debug)]
pub enum PeerState {
    /// [`PeerStat`] is started.
    Started(HashSet<FlowMetricSource>),

    /// [`PeerStat`] is stopped.
    Stopped,
}

/// Current state of `PeerConnection`.
///
/// Also this structure may be considered as subscription to Control API
/// callbacks.
#[derive(Debug)]
pub struct PeerStat {
    /// [`PeerId`] of `PeerConnection` which this [`PeerStat`] represents.
    pub peer_id: PeerId,

    /// Current state of this [`PeerStat`].
    pub state: PeerState,

    /// Time of last received [`PeerState`] proof.
    ///
    /// If [`PeerStat`] doesn't updates withing `10secs` then this [`PeerStat`]
    /// will be considered as [`PeerState::Stopped`].
    pub last_update: Instant,
}

/// Stores [`PeerStat`]s of `PeerConnection`s for which [`PeerState`] [`Room`]
/// is watching.
#[derive(Debug)]
pub struct RoomStats {
    /// [`RoomId`] of all [`PeerStat`] which stored here.
    room_id: RoomId,

    /// [`Addr`] of [`Room`] which is watching for this [`PeerStat`]s.
    room: Addr<Room>,

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
pub struct RegisterRoom {
    /// [`RoomId`] of [`Room`] which requested to register in the
    /// [`MetricsCallbacksService`].
    pub room_id: RoomId,

    /// [`Addr`] of room which requrested to register in the
    /// [`MetricsCallbackService`].
    pub room: Addr<Room>,
}

impl Handler<RegisterRoom> for MetricsCallbacksService {
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

/// Unregister [`Room`] with provided [`RoomId`] from the
/// [`MetricsCallbacksService`].
///
/// This message will just remove subscription. This isn't considered as
/// [`TrafficStopped`] or something like this.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct UnregisterRoom(pub RoomId);

impl Handler<UnregisterRoom> for MetricsCallbacksService {
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
pub struct SubscribePeer {
    /// [`RoomId`] of [`Room`] which subscribes on [`PeerStat`]'s [`PeerState`]
    /// changes.
    pub room_id: RoomId,

    /// [`PeerId`] of [`PeerStat`] for which subscription is requested.
    pub peer_id: PeerId,
}

impl Handler<SubscribePeer> for MetricsCallbacksService {
    type Result = ();

    fn handle(
        &mut self,
        msg: SubscribePeer,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            room.peers.insert(
                msg.peer_id,
                PeerStat {
                    peer_id: msg.peer_id,
                    state: PeerState::Stopped,
                    last_update: Instant::now(),
                },
            );
        }
    }
}

/// Unsubscribes [`Room`] with provided [`RoomId`] from [`PeerStat`] with
/// provided [`PeerId`].
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct UnsubscribePeers {
    /// [`RoomId`] of [`Room`] which unsubscribes from [`PeerStat`]'s
    /// [`PeerState`] changes.
    pub room_id: RoomId,

    /// [`PeerId`] of [`PeerStat`] from which unsubscription is requested.
    pub peers_ids: HashSet<PeerId>,
}

impl Handler<UnsubscribePeers> for MetricsCallbacksService {
    type Result = ();

    fn handle(
        &mut self,
        msg: UnsubscribePeers,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room_stats) = self.stats.get_mut(&msg.room_id) {
            for peer_id in msg.peers_ids {
                room_stats.peers.remove(&peer_id);
            }
        }
    }
}
