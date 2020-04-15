//! Service which is responsible for processing [`PeerConnection`]'s metrics
//! received from a client.

use std::{
    cell::RefCell,
    collections::HashMap,
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

use crate::{
    api::control::RoomId,
    media::peer::{New, Peer},
};

use super::peers_traffic_watcher::{
    FlowMetricSource, PeerTrafficWatcher, StoppedMetricSource,
};

/// Media type of a [`MediaTrack`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
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
    /// Fatal `PeerConnection`'s contradiction of metrics with specification.
    ///
    /// On this [`PeerMetricsEvent`] `PeerConnection` with provided [`PeerId`]
    /// should be stopped.
    FatalPeerFailure { peer_id: PeerId, at: DateTime<Utc> },
}

/// Specification of `PeerConnection`.
///
/// Based on this specification fatal `PeerConnection`'s errors will be
/// determined.
#[derive(Debug)]
pub struct PeerSpec {
    /// All `MediaTrack`s with `Send` direction of `PeerConnection`.
    ///
    /// Order isn't important.
    pub senders: Vec<TrackMediaType>,

    /// All `MediaTrack`s with `Recv` direction of `PeerConnection`.
    ///
    /// Order isn't important.
    pub receivers: Vec<TrackMediaType>,
}

/// Metrics which are available for `MediaTrack` with `Send` direction.
#[derive(Debug)]
struct SenderStat {
    /// Last time when this stat was updated.
    last_update: Instant,

    /// Count of packets sent by a `MediaTrack` which this [`SenderStat`]
    /// represents.
    packets_sent: u64,

    /// Media type of a `MediaTrack` which this [`SenderStat`] represents.
    media_type: TrackMediaType,
}

impl SenderStat {
    /// Updates this [`SenderStat`] with provided [`RtcOutboundRtpStreamStats`].
    ///
    /// [`SenderStat::last_update`] time will be updated.
    fn update(&mut self, upd: &RtcOutboundRtpStreamStats) {
        self.last_update = Instant::now();
        self.packets_sent = upd.packets_sent;
    }
}

/// Metrics which is available for `MediaTrack` with `Recv` direction.
#[derive(Debug)]
struct ReceiverStat {
    /// Last time when this stat was updated.
    last_update: Instant,

    /// Count of packets received by a `MediaTrack` which this [`ReceiverStat`]
    /// represents.
    packets_received: u64,

    /// Media type of a `MediaTrack` which this [`ReceiverStat`] represents.
    media_type: TrackMediaType,
}

impl ReceiverStat {
    /// Updates this [`SenderStat`] with provided [`RtcOutboundRtpStreamStats`].
    ///
    /// [`SenderStat::last_update`] time will be updated.
    fn update(&mut self, upd: &RtcInboundRtpStreamStats) {
        self.last_update = Instant::now();
        self.packets_received = upd.packets_received;
    }
}

/// Current state of a [`PeerStat`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerStatState {
    /// `PeerConnection` which this [`PeerStat`] represents is considered as
    /// connected.
    Connected,

    /// `PeerConnection` which this [`PeerStat`] represents waiting for
    /// connection.
    Waiting,
}

/// Current metrics of some `PeerConnection`.
#[derive(Debug)]
struct PeerStat {
    /// [`PeerId`] of `PeerConnection` which this [`PeerStat`] represents.
    peer_id: PeerId,

    /// Weak reference to a [`PeerStat`] which reprensents partner
    /// `PeerConnection`.
    partner_peer: Weak<RefCell<PeerStat>>,

    /// Specification of a `PeerConnection` which this [`PeerStat`] represents.
    spec: PeerSpec,

    /// All [`SenderStat`]s of this [`PeerStat`].
    senders: HashMap<StatId, SenderStat>,

    /// All [`ReceiverStat`]s of this [`PeerStat`].
    receivers: HashMap<StatId, ReceiverStat>,

    /// Current state of this [`PeerStat`].
    state: PeerStatState,

    /// Time of the last metrics update of this [`PeerStat`].
    last_update: DateTime<Utc>,

    peer_validity_timeout: Duration,
}

impl PeerStat {
    /// Updates [`SenderStat`] with provided [`StatId`] by
    /// [`RtcOutboundRtpStreamStats`].
    fn update_sender(
        &mut self,
        stat_id: StatId,
        upd: &RtcOutboundRtpStreamStats,
    ) {
        self.last_update = Utc::now();
        self.senders
            .entry(stat_id)
            .or_insert_with(|| SenderStat {
                last_update: Instant::now(),
                packets_sent: 0,
                media_type: TrackMediaType::from(&upd.media_type),
            })
            .update(upd);
    }

    /// Updates [`ReceiverStat`] with provided [`StatId`] by
    /// [`RtcInboundRtpStreamStats`].
    fn update_receiver(
        &mut self,
        stat_id: StatId,
        upd: &RtcInboundRtpStreamStats,
    ) {
        self.last_update = Utc::now();
        self.receivers
            .entry(stat_id)
            .or_insert_with(|| ReceiverStat {
                last_update: Instant::now(),
                packets_received: 0,
                media_type: TrackMediaType::from(&upd.media_specific_stats),
            })
            .update(upd);
    }

    // TODO: docs
    fn is_track_active(&self, last_update: Instant) -> bool {
        last_update > Instant::now() - self.peer_validity_timeout
    }

    /// Checks that this [`PeerStat`] is conforms to `PeerConnection`
    /// specification.
    ///
    /// This is determined by comparing count of senders/receivers from the
    /// `PeerConnection` specification. Also media type of sender/receiver
    /// and activity taken into account.
    #[allow(clippy::filter_map)]
    fn is_conforms_spec(&self) -> bool {
        let mut spec_senders: Vec<_> = self.spec.senders.clone();
        let mut spec_receivers: Vec<_> = self.spec.receivers.clone();
        spec_senders.sort();
        spec_receivers.sort();

        let mut current_senders: Vec<_> = self
            .senders
            .values()
            .filter(|sender| self.is_track_active(sender.last_update))
            .map(|sender| sender.media_type)
            .collect();
        let mut current_receivers: Vec<_> = self
            .receivers
            .values()
            .filter(|receiver| self.is_track_active(receiver.last_update))
            .map(|receiver| receiver.media_type)
            .collect();
        current_receivers.sort();
        current_senders.sort();

        spec_receivers == current_receivers && spec_senders == current_senders
    }

    /// Returns `true` if all senders and receivers is not sending or receiving
    /// anything.
    fn is_stopped(&self) -> bool {
        let active_senders_count = self
            .senders
            .values()
            .filter(|sender| self.is_track_active(sender.last_update))
            .count();
        let active_receivers_count = self
            .receivers
            .values()
            .filter(|recv| self.is_track_active(recv.last_update))
            .count();

        active_receivers_count + active_senders_count == 0
    }

    /// Returns time of stat which haven't updated longest.
    fn get_stop_time(&self) -> Instant {
        self.senders
            .values()
            .map(|send| send.last_update)
            .chain(self.receivers.values().map(|recv| recv.last_update))
            .min()
            .unwrap_or_else(Instant::now)
    }

    /// Returns `Some` [`PeerId`] of a partner `PeerConnection` if partner
    /// [`PeerStat`]'s weak pointer is available.
    ///
    /// Returns `None` if weak pointer of partner [`PeerStat`] is unavailable.
    fn get_partner_peer_id(&self) -> Option<PeerId> {
        self.partner_peer
            .upgrade()
            .map(|partner_peer| partner_peer.borrow().get_peer_id())
    }

    /// Returns [`PeerId`] of `PeerConnection` which this [`PeerStat`]
    /// represents.
    fn get_peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Sets new [`PeerStatState`] for this [`PeerStat`].
    fn set_state(&mut self, state: PeerStatState) {
        self.state = state;
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
    pub fn check_peers_validity(&self) {
        for peer in self
            .peers
            .values()
            .filter(|peer| peer.borrow().state == PeerStatState::Connected)
        {
            let peer_ref = peer.borrow();

            if peer_ref.is_stopped() {
                self.peers_traffic_watcher.traffic_stopped(
                    self.room_id.clone(),
                    peer_ref.peer_id,
                    peer_ref.get_stop_time(),
                    StoppedMetricSource::PeerTraffic,
                );
            } else if !peer_ref.is_conforms_spec() {
                self.fatal_peer_error(peer_ref.peer_id, Utc::now());
            }
        }
    }

    /// [`Room`] notifies [`PeersMetrics`] about new `PeerConnection`s
    /// creation.
    ///
    /// Based on the provided [`PeerSpec`]s [`PeerStat`]s will be validated.
    pub fn add_peers(
        &mut self,
        first_peer: &Peer<New>,
        second_peer: &Peer<New>,
        peer_validity_timeout: Duration,
    ) {
        let first_peer_stat = Rc::new(RefCell::new(PeerStat {
            peer_id: first_peer.id(),
            partner_peer: Weak::new(),
            last_update: Utc::now(),
            senders: HashMap::new(),
            receivers: HashMap::new(),
            state: PeerStatState::Waiting,
            spec: first_peer.get_spec(),
            peer_validity_timeout,
        }));
        let second_peer_stat = Rc::new(RefCell::new(PeerStat {
            peer_id: second_peer.id(),
            partner_peer: Rc::downgrade(&first_peer_stat),
            last_update: Utc::now(),
            senders: HashMap::new(),
            receivers: HashMap::new(),
            state: PeerStatState::Waiting,
            spec: second_peer.get_spec(),
            peer_validity_timeout,
        }));
        first_peer_stat.borrow_mut().partner_peer =
            Rc::downgrade(&second_peer_stat);

        self.peers.insert(first_peer.id(), first_peer_stat);
        self.peers.insert(second_peer.id(), second_peer_stat);
    }

    /// Adds new [`RtcStat`]s for the [`PeerStat`]s from this
    /// [`PeersMetrics`].
    pub fn add_stat(&mut self, peer_id: PeerId, stats: Vec<RtcStat>) {
        if let Some(peer) = self.peers.get(&peer_id) {
            let mut peer_ref = peer.borrow_mut();

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
                self.peers_traffic_watcher.traffic_stopped(
                    self.room_id.clone(),
                    peer_ref.peer_id,
                    peer_ref.get_stop_time(),
                    StoppedMetricSource::PeerTraffic,
                );
            } else if peer_ref.is_conforms_spec() {
                self.peers_traffic_watcher.traffic_flows(
                    self.room_id.clone(),
                    peer_id,
                    Instant::now(),
                    FlowMetricSource::Peer,
                );
                peer_ref.set_state(PeerStatState::Connected);
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
    ///
    /// This will be considered as [`TrafficStopped`] for the
    /// [`PeersTrafficWatcher`].
    pub fn peer_removed(&mut self, peer_id: PeerId) {
        if self.peers.remove(&peer_id).is_some() {
            // TODO: seems redundant
            self.peers_traffic_watcher.traffic_stopped(
                self.room_id.clone(),
                peer_id,
                Instant::now(),
                StoppedMetricSource::PeerRemoved,
            );
        }
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
