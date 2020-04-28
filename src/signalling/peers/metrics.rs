//! Service which is responsible for processing [`Peer`]'s [`RtcStat`] metrics.
//!
//! This service acts as flow and stop metrics source for the
//! [`PeerTrafficWatcher`].

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use futures::{channel::mpsc, Stream};
use medea_client_api_proto::{
    stats::{
        RtcInboundRtpStreamMediaType, RtcInboundRtpStreamStats,
        RtcOutboundRtpStreamMediaType, RtcOutboundRtpStreamStats, RtcStat,
        RtcStatsType, StatId,
    },
    PeerId,
};
use medea_macro::dispatchable;

use crate::{api::control::RoomId, log::prelude::*, media::PeerStateMachine};

use super::traffic_watcher::{FlowMetricSource, PeerTrafficWatcher};

/// Media type of a [`MediaTrack`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub enum TrackMediaType {
    Audio,
    Video,
}

/// Events which [`PeersMetrics`] can throw to the
/// [`PeersMetrics::peer_metric_events_sender`]'s receiver (currently this
/// is [`Room`] which owns this [`PeersMetrics`]).
#[dispatchable]
#[derive(Debug, Clone)]
pub enum PeersMetricsEvent {
    /// Fatal [`Peer`]'s contradiction of the client metrics with
    /// [`PeerSpec`].
    FatalPeerFailure { peer_id: PeerId, at: DateTime<Utc> },
}

/// Specification of a [`Peer`].
///
/// Based on this specification fatal [`Peer`]'s media traffic contradictions
/// will be determined.
#[derive(Debug)]
pub struct PeerSpec {
    /// All `MediaTrack`s with `Send` direction of [`Peer`].
    ///
    /// Value - count of `MediaTrack`s with this [`TrackMediaType`].
    pub senders: HashMap<TrackMediaType, u64>,

    /// All `MediaTrack`s with `Recv` direction of [`Peer`].
    ///
    /// Value - count of `MediaTrack`s with this [`TrackMediaType`].
    pub receivers: HashMap<TrackMediaType, u64>,
}

/// Metrics which are available for `MediaTrack` with `Send` direction.
#[derive(Debug)]
struct SendDir {
    /// Count of packets sent by a `MediaTrack` which this [`TrackStat`]
    /// represents.
    packets_sent: u64,
}

/// Metrics which are available for `MediaTrack` with `Recv` direction.
#[derive(Debug)]
struct RecvDir {
    /// Count of packets received by a `MediaTrack` which this [`TrackStat`]
    /// represents.
    packets_received: u64,
}

/// Metrics of the `MediaTrack` with [`SendDir`] or [`RecvDir`] state.
#[derive(Debug)]
struct TrackStat<T> {
    /// Last time when this [`TrackStat`] was updated.
    last_update: Instant,

    /// Media type of the `MediaTrack` which this [`TrackStat`] represents.
    media_type: TrackMediaType,

    /// Direction state of this [`TrackStat`].
    ///
    /// Can be [`SendDir`] or [`RecvDir`].
    direction: T,
}

impl<T> TrackStat<T> {
    /// Returns [`Instant`] time on which this [`TrackStat`] was updated last
    /// time.
    fn last_update(&self) -> &Instant {
        &self.last_update
    }
}

impl TrackStat<SendDir> {
    /// Updates this [`TrackStat`] with provided [`RtcOutboundRtpStreamStats`].
    ///
    /// [`TrackStat::last_update`] time will be updated.
    fn update(&mut self, upd: &RtcOutboundRtpStreamStats) {
        self.last_update = Instant::now();
        self.direction.packets_sent = upd.packets_sent;
    }
}

impl TrackStat<RecvDir> {
    /// Updates this [`TrackStat`] with provided [`RtcInboundRtpStreamStats`].
    ///
    /// [`TrackStat::last_update`] time will be updated.
    fn update(&mut self, upd: &RtcInboundRtpStreamStats) {
        self.last_update = Instant::now();
        self.direction.packets_received = upd.packets_received;
    }
}

/// Current state of a [`PeerStat`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerStatState {
    /// [`Peer`] which this [`PeerStat`] represents is considered as
    /// connected.
    Connected,

    /// [`Peer`] which this [`PeerStat`] represents waiting for
    /// connection.
    Connecting,
}

/// Current stats of some [`Peer`].
#[derive(Debug)]
struct PeerStat {
    /// [`PeerId`] of [`Peer`] which this [`PeerStat`] represents.
    peer_id: PeerId,

    /// Weak reference to a [`PeerStat`] which represents a partner
    /// [`Peer`].
    partner_peer: Weak<RefCell<PeerStat>>,

    /// Specification of a [`Peer`] which this [`PeerStat`] represents.
    spec: PeerSpec,

    /// All [`TrackStat`]s with [`Send`] direction of this [`PeerStat`].
    senders: HashMap<StatId, TrackStat<SendDir>>,

    /// All [`TrackStat`]s with [`Recv`] of this [`PeerStat`].
    receivers: HashMap<StatId, TrackStat<RecvDir>>,

    /// Current connection state of this [`PeerStat`].
    state: PeerStatState,

    /// Time of the last metrics update of this [`PeerStat`].
    last_update: DateTime<Utc>,

    /// [`Duration`] after which media server will consider this [`Peer`]'s
    /// media traffic stats as invalid and will remove this [`Peer`].
    peer_validity_timeout: Duration,
}

impl PeerStat {
    /// Updates [`TrackStat`] with provided [`StatId`] by
    /// [`RtcOutboundRtpStreamStats`].
    fn update_sender(
        &mut self,
        stat_id: StatId,
        upd: &RtcOutboundRtpStreamStats,
    ) {
        self.last_update = Utc::now();
        self.senders
            .entry(stat_id)
            .or_insert_with(|| TrackStat {
                last_update: Instant::now(),
                direction: SendDir { packets_sent: 0 },
                media_type: TrackMediaType::from(&upd.media_type),
            })
            .update(upd);
    }

    /// Updates [`TrackStat`] with provided [`StatId`] by
    /// [`RtcInboundRtpStreamStats`].
    fn update_receiver(
        &mut self,
        stat_id: StatId,
        upd: &RtcInboundRtpStreamStats,
    ) {
        self.last_update = Utc::now();
        self.receivers
            .entry(stat_id)
            .or_insert_with(|| TrackStat {
                last_update: Instant::now(),
                direction: RecvDir {
                    packets_received: 0,
                },
                media_type: TrackMediaType::from(&upd.media_specific_stats),
            })
            .update(upd);
    }

    /// Checks that media traffic flows through provided [`TrackStat`].
    ///
    /// [`TrackStat`] should be updated within
    /// [`PeerStat::peer_validity_timeout`] or this [`TrackStat`] will be
    /// considered as stopped.
    fn is_track_active<T>(&self, track: &TrackStat<T>) -> bool {
        track.last_update().elapsed() < self.peer_validity_timeout
    }

    /// Checks that this [`PeerStat`] is conforms to [`PeerSpec`].
    ///
    /// This is determined by comparing count of senders/receivers from the
    /// [`PeerSpec`].
    ///
    /// Also media type of sender/receiver
    /// and activity taken into account.
    fn is_conforms_spec(&self) -> bool {
        let mut current_senders = HashMap::new();
        let mut current_receivers = HashMap::new();

        self.senders
            .values()
            .filter(|t| self.is_track_active(&t))
            .for_each(|sender| {
                *current_senders.entry(sender.media_type).or_insert(0) += 1;
            });
        self.receivers
            .values()
            .filter(|t| self.is_track_active(&t))
            .for_each(|receiver| {
                *current_receivers.entry(receiver.media_type).or_insert(0) += 1;
            });

        for (receivers_type, receiver_count) in &self.spec.receivers {
            if let Some(spec_count) = current_receivers.get(receivers_type) {
                if spec_count < receiver_count {
                    return false;
                }
            }
        }
        for (senders_type, senders_count) in &self.spec.senders {
            if let Some(spec_count) = current_senders.get(senders_type) {
                if spec_count < senders_count {
                    return false;
                }
            }
        }

        true
    }

    /// Returns `true` if all senders and receivers is not sending or receiving
    /// anything.
    fn is_stopped(&self) -> bool {
        let active_senders_count = self
            .senders
            .values()
            .filter(|sender| self.is_track_active(&sender))
            .count();
        let active_receivers_count = self
            .receivers
            .values()
            .filter(|recv| self.is_track_active(&recv))
            .count();

        active_receivers_count + active_senders_count == 0
    }

    /// Returns time of [`TrackStat`] which haven't updated longest.
    fn get_stop_time(&self) -> Instant {
        self.senders
            .values()
            .map(|send| send.last_update)
            .chain(self.receivers.values().map(|recv| recv.last_update))
            .min()
            .unwrap_or_else(Instant::now)
    }

    /// Returns `Some` [`PeerId`] of a partner [`Peer`] if partner
    /// [`PeerStat`]'s weak pointer is available.
    ///
    /// Returns `None` if weak pointer of partner [`PeerStat`] is unavailable.
    fn get_partner_peer_id(&self) -> Option<PeerId> {
        self.partner_peer
            .upgrade()
            .map(|partner_peer| partner_peer.borrow().get_peer_id())
    }

    /// Returns [`PeerId`] of [`Peer`] which this [`PeerStat`]
    /// represents.
    fn get_peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Sets state of this [`PeerStat`] to [`PeerStatState::Connected`].
    fn connected(&mut self) {
        self.state = PeerStatState::Connected;
    }
}

/// Service which responsible for processing [`PeerConnection`]'s metrics
/// received from a client.
#[derive(Debug)]
pub struct PeersMetricsService {
    /// [`RoomId`] of [`Room`] to which this [`PeersMetrics`] belongs to.
    room_id: RoomId,

    /// [`Addr`] of [`PeersTrafficWatcher`] to which traffic updates will be
    /// sent.
    peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,

    /// All `PeerConnection` for this [`PeersMetrics`] will process
    /// metrics.
    peers: HashMap<PeerId, Rc<RefCell<PeerStat>>>,

    /// Sender of [`PeerMetricsEvent`]s.
    ///
    /// Currently [`PeerMetricsEvent`] will receive [`Room`] to which this
    /// [`PeersMetrics`] belongs to.
    peer_metric_events_sender: Option<mpsc::UnboundedSender<PeersMetricsEvent>>,
}

impl PeersMetricsService {
    /// Returns new [`PeersMetrics`] for a provided [`Room`].
    pub fn new(
        room_id: RoomId,
        peers_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
    ) -> Self {
        Self {
            room_id,
            peers_traffic_watcher,
            peers: HashMap::new(),
            peer_metric_events_sender: None,
        }
    }

    /// Some fatal error with `PeerConnection`'s metrics happened.
    ///
    /// [`PeerMetricsEvent::FatalPeerFailure`] will be sent to the subscriber.
    fn fatal_peer_error(&self, peer_id: PeerId, at: DateTime<Utc>) {
        if let Some(sender) = &self.peer_metric_events_sender {
            let _ =
                sender.unbounded_send(PeersMetricsEvent::FatalPeerFailure {
                    peer_id,
                    at,
                });
        }
    }

    /// Returns [`Stream`] of [`PeerMetricsEvent`]s.
    ///
    /// Currently this method will be called by a [`Room`] to which this
    /// [`PeersMetrics`] belongs to.
    pub fn subscribe(&mut self) -> impl Stream<Item = PeersMetricsEvent> {
        let (tx, rx) = mpsc::unbounded();
        self.peer_metric_events_sender = Some(tx);

        rx
    }

    /// Checks that all [`PeerStat`]s is valid accordingly `PeerConnection`
    /// specification. If [`PeerStat`] is considered as invalid accordingly to
    /// `PeerConnection` specification then
    /// [`PeersMetrics::fatal_peer_error`] will be called.
    ///
    /// Also checks that all [`PeerStat`]'s senders/receivers is flowing. If all
    /// senders/receivers is stopped then [`TrafficStopped`] will be sent to
    /// the [`PeersTrafficWatcher`].
    pub fn check_peers_validity(&mut self) {
        let mut stopped_peers = Vec::new();
        for peer in self
            .peers
            .values()
            .filter(|peer| peer.borrow().state == PeerStatState::Connected)
        {
            let peer_ref = peer.borrow();

            if peer_ref.is_stopped() {
                debug!(
                    "Peer [id = {}] from Room [id = {}] traffic stopped \
                     because all his traffic not flowing.",
                    peer_ref.peer_id, self.room_id
                );
                self.peers_traffic_watcher.traffic_stopped(
                    self.room_id.clone(),
                    peer_ref.peer_id,
                    peer_ref.get_stop_time(),
                );
                stopped_peers.push(peer_ref.peer_id);
            } else if !peer_ref.is_conforms_spec() {
                debug!(
                    "Peer [id = {}] from Room [id = {}] traffic stopped \
                     because invalid traffic flowing.",
                    peer_ref.peer_id, self.room_id
                );
                self.fatal_peer_error(peer_ref.peer_id, Utc::now());
            }
        }

        for stopped_peer_id in stopped_peers {
            self.peers.remove(&stopped_peer_id);
        }
    }

    /// [`Room`] notifies [`PeersMetrics`] about new `PeerConnection`s
    /// creation.
    ///
    /// Based on the provided [`PeerSpec`]s [`PeerStat`]s will be validated.
    pub fn register_peer(
        &mut self,
        peer: &PeerStateMachine,
        peer_validity_timeout: Duration,
    ) {
        debug!(
            "Peer [id = {}] was registered in the PeerMetricsService [room_id \
             = {}].",
            peer.id(),
            self.room_id
        );

        let first_peer_stat = Rc::new(RefCell::new(PeerStat {
            peer_id: peer.id(),
            partner_peer: Weak::new(),
            last_update: Utc::now(),
            senders: HashMap::new(),
            receivers: HashMap::new(),
            state: PeerStatState::Connecting,
            spec: peer.get_spec(),
            peer_validity_timeout,
        }));
        if let Some(partner_peer_stat) = self.peers.get(&peer.partner_peer_id())
        {
            first_peer_stat.borrow_mut().partner_peer =
                Rc::downgrade(&partner_peer_stat);
            partner_peer_stat.borrow_mut().partner_peer =
                Rc::downgrade(&first_peer_stat);
        }

        self.peers.insert(peer.id(), first_peer_stat);
    }

    /// Adds new [`RtcStat`]s for the [`PeerStat`]s from this
    /// [`PeersMetrics`].
    pub fn add_stat(&mut self, peer_id: PeerId, stats: Vec<RtcStat>) {
        if let Some(peer) = self.peers.get(&peer_id) {
            let mut peer_ref = peer.borrow_mut();
            let is_conforms_spec_before_upd = peer_ref.is_conforms_spec();

            for stat in stats {
                match &stat.stats {
                    RtcStatsType::InboundRtp(inbound) => {
                        peer_ref.update_receiver(stat.id, inbound);
                    }
                    RtcStatsType::OutboundRtp(outbound) => {
                        peer_ref.update_sender(stat.id, outbound);
                    }
                    _ => (),
                }
            }

            if peer_ref.is_stopped() {
                debug!(
                    "Peer [id = {}] from Room [id = {}] traffic stopped \
                     because traffic stats doesn't updated too long.",
                    peer_ref.peer_id, self.room_id
                );
                self.peers_traffic_watcher.traffic_stopped(
                    self.room_id.clone(),
                    peer_ref.peer_id,
                    peer_ref.get_stop_time(),
                );
            } else if peer_ref.is_conforms_spec() {
                if !is_conforms_spec_before_upd {
                    peer_ref.connected();
                }
                self.peers_traffic_watcher.traffic_flows(
                    self.room_id.clone(),
                    peer_id,
                    Instant::now(),
                    FlowMetricSource::Peer,
                );
                if let Some(partner_peer_id) = peer_ref.get_partner_peer_id() {
                    self.peers_traffic_watcher.traffic_flows(
                        self.room_id.clone(),
                        partner_peer_id,
                        Instant::now(),
                        FlowMetricSource::PartnerPeer,
                    );
                }
            } else {
                self.fatal_peer_error(peer_ref.peer_id, peer_ref.last_update);
            }
        }
    }

    /// [`Room`] notifies [`PeersMetrics`] that some [`Peer`] is removed.
    pub fn unregister_peers(&mut self, peers_ids: HashSet<PeerId>) {
        debug!(
            "Peers [ids = [{:?}]] from Room [id = {}] was unsubscribed from \
             the PeerMetricsService.",
            peers_ids, self.room_id
        );

        for peer_id in &peers_ids {
            self.peers.remove(peer_id);
        }
        self.peers_traffic_watcher
            .unregister_peers(self.room_id.clone(), peers_ids);
    }

    pub fn update_peer_spec(&mut self, peer_id: PeerId, spec: PeerSpec) {
        if let Some(peer) = self.peers.get(&peer_id) {
            let mut peer_ref = peer.borrow_mut();
            debug!(
                "Spec of a Peer [id = {}] was updated. Spec before: {:?}. \
                 Spec after: {:?}.",
                peer_id, peer_ref.spec, spec
            );
            peer_ref.spec = spec;
        }
    }

    pub fn is_peer_registered(&self, peer_id: PeerId) -> bool {
        self.peers.contains_key(&peer_id)
    }
}

impl From<&RtcOutboundRtpStreamMediaType> for TrackMediaType {
    fn from(from: &RtcOutboundRtpStreamMediaType) -> Self {
        match from {
            RtcOutboundRtpStreamMediaType::Audio { .. } => Self::Audio,
            RtcOutboundRtpStreamMediaType::Video { .. } => Self::Video,
        }
    }
}

impl From<&RtcInboundRtpStreamMediaType> for TrackMediaType {
    fn from(from: &RtcInboundRtpStreamMediaType) -> Self {
        match from {
            RtcInboundRtpStreamMediaType::Audio { .. } => Self::Audio,
            RtcInboundRtpStreamMediaType::Video { .. } => Self::Video,
        }
    }
}

impl From<&medea_client_api_proto::MediaType> for TrackMediaType {
    fn from(from: &medea_client_api_proto::MediaType) -> Self {
        match from {
            medea_client_api_proto::MediaType::Audio(_) => Self::Audio,
            medea_client_api_proto::MediaType::Video(_) => Self::Video,
        }
    }
}
