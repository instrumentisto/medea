//! Service is which responsible for [`PeerConnection`]'s metrics processing.

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use actix::Addr;
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

use crate::api::control::{
    callback::metrics_callback_service::{
        FlowMetricSource, MetricsCallbacksService, StoppedMetricSource,
        TrafficFlows, TrafficStopped,
    },
    RoomId,
};

/// Media type of a [`MediaTrack`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub enum TrackMediaType {
    Audio,
    Video,
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

/// Events which [`PeerMetricsService`] can throw to the
/// [`PeerMetricsService::peer_metric_events_sender`]'s receiver (currently this
/// is [`Room`] which owns this [`PeerMetricsService`]).
#[dispatchable]
#[derive(Debug, Clone)]
pub enum PeerMetricsEvent {
    /// Fatal `PeerConnection`'s contradiction of metrics with specification.
    ///
    /// On this [`PeerMetricsEvent`] `PeerConnection` with provided [`PeerId`]
    /// should be stopped.
    FatalPeerFailure(PeerId),
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
    pub received: Vec<TrackMediaType>,
}

/// Metrics which is available for `MediaTrack` with `Send` direction.
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

    /// Checks that this [`SenderStat`] is active.
    ///
    /// This will be calculated by checking that this [`SenderStat`] was updated
    /// within `10secs`.
    fn is_active(&self) -> bool {
        self.last_update > Instant::now() - Duration::from_secs(10)
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

    /// Checks that this [`ReceiverStat`] is active.
    ///
    /// This will be calculated by checking that this [`ReceiverStat`] was
    /// updated within `10secs`.
    fn is_active(&self) -> bool {
        self.last_update > Instant::now() - Duration::from_secs(10)
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
    last_update: Instant,
}

impl PeerStat {
    /// Updates [`SenderStat`] with provided [`StatId`] by
    /// [`RtcOutboundRtpStreamStats`].
    fn update_sender(
        &mut self,
        stat_id: StatId,
        upd: &RtcOutboundRtpStreamStats,
    ) {
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
        self.receivers
            .entry(stat_id)
            .or_insert_with(|| ReceiverStat {
                last_update: Instant::now(),
                packets_received: 0,
                media_type: TrackMediaType::from(&upd.media_specific_stats),
            })
            .update(upd);
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
        let mut spec_receivers: Vec<_> = self.spec.received.clone();
        spec_senders.sort();
        spec_receivers.sort();

        let mut current_senders: Vec<_> = self
            .senders
            .values()
            .filter(|sender| sender.is_active())
            .map(|sender| sender.media_type)
            .collect();
        let mut current_receivers: Vec<_> = self
            .receivers
            .values()
            .filter(|receiver| receiver.is_active())
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
            .filter(|sender| sender.is_active())
            .count();
        let active_receivers_count = self
            .receivers
            .values()
            .filter(|recv| recv.is_active())
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

/// New `PeerConnection` for which this [`PeerMetricsService`] will receive
/// metrics.
#[derive(Debug)]
pub struct Peer {
    /// [`PeerId`] of `PeerConnection` for which this [`PeerMetricsService`]
    /// will receive metrics.
    pub peer_id: PeerId,

    /// Specification of a `PeerConnection` for which this
    /// [`PeerMetricsService`] will receive metrics.
    pub spec: PeerSpec,
}

/// Service which responsible for [`PeerConnection`]'s metrics processing.
#[derive(Debug)]
pub struct PeerMetricsService {
    /// [`RoomId`] of [`Room`] to which this [`PeerMetricsService`] belongs to.
    room_id: RoomId,

    /// [`Addr`] of [`MetricsCallbackService`] to which traffic updates will be
    /// sent.
    metrics_service: Addr<MetricsCallbacksService>,

    /// All `PeerConnection` for this this [`PeerMetricsService`] will proccess
    /// metrics.
    peers: HashMap<PeerId, Rc<RefCell<PeerStat>>>,

    /// Sender of [`PeerMetricsEvent`]s.
    ///
    /// Currently [`PeerMetricsEvent`] will receive [`Room`] to which this
    /// [`PeerMetricsService`] belongs to.
    peer_metric_events_sender: Option<mpsc::UnboundedSender<PeerMetricsEvent>>,
}

impl PeerMetricsService {
    /// Returns new [`PeerMetricsService`] for provided [`Room`].
    pub fn new(
        room_id: RoomId,
        metrics_service: Addr<MetricsCallbacksService>,
    ) -> Self {
        Self {
            room_id,
            metrics_service,
            peers: HashMap::new(),
            peer_metric_events_sender: None,
        }
    }

    /// Some fatal error with `PeerConnection`'s metrics happened.
    ///
    /// [`PeerMetricsEvent::FatalPeerFailure`] will be sent to the subscriber.
    fn fatal_peer_error(&self, peer_id: PeerId) {
        if let Some(sender) = &self.peer_metric_events_sender {
            let _ = sender
                .unbounded_send(PeerMetricsEvent::FatalPeerFailure(peer_id));
        }
    }

    /// Returns [`Stream`] of [`PeerMetricsEvent`]s.
    ///
    /// Currently this method will be called by [`Room`] to which this
    /// [`PeerMetricsService`] belongs to.
    pub fn subscribe(&mut self) -> impl Stream<Item = PeerMetricsEvent> {
        let (tx, rx) = mpsc::unbounded();
        self.peer_metric_events_sender = Some(tx);

        rx
    }

    /// Checks that all [`PeerStat`]s is valid accordingly `PeerConnection`
    /// specification. If [`PeerStat`] is considered as invalid accrdingly to
    /// `PeerConnection` specification then
    /// [`PeerMetricsService::fatal_peer_error`] will be called.
    ///
    /// Also checks that all [`PeerStat`]'s senders/receivers is flowing. If all
    /// senders/receivers is stopped then [`TrafficStopped`] will be sent to
    /// the [`MetricsCallbackService`].
    pub fn check_peers_validity(&self) {
        for peer in self
            .peers
            .values()
            .filter(|peer| peer.borrow().state == PeerStatState::Connected)
        {
            let peer_ref = peer.borrow();

            if peer_ref.is_stopped() {
                self.metrics_service.do_send(TrafficStopped {
                    room_id: self.room_id.clone(),
                    peer_id: peer_ref.peer_id,
                    timestamp: peer_ref.get_stop_time(),
                    source: StoppedMetricSource::PeerTraffic,
                });
            } else if !peer_ref.is_conforms_spec() {
                self.fatal_peer_error(peer_ref.peer_id);
            }
        }
    }

    /// [`Room`] notifies [`PeerMetricsService`] about new `PeerConnection`s
    /// creation.
    ///
    /// Based on the provided [`PeerSpec`]s [`PeerStat`]s will be validated.
    pub fn add_peers(&mut self, first_peer: Peer, second_peer: Peer) {
        let first_peer_stat = Rc::new(RefCell::new(PeerStat {
            peer_id: first_peer.peer_id,
            partner_peer: Weak::new(),
            last_update: Instant::now(),
            senders: HashMap::new(),
            receivers: HashMap::new(),
            state: PeerStatState::Waiting,
            spec: first_peer.spec,
        }));
        let second_peer_stat = Rc::new(RefCell::new(PeerStat {
            peer_id: second_peer.peer_id,
            partner_peer: Rc::downgrade(&first_peer_stat),
            last_update: Instant::now(),
            senders: HashMap::new(),
            receivers: HashMap::new(),
            state: PeerStatState::Waiting,
            spec: second_peer.spec,
        }));
        first_peer_stat.borrow_mut().partner_peer =
            Rc::downgrade(&second_peer_stat);

        self.peers.insert(first_peer.peer_id, first_peer_stat);
        self.peers.insert(second_peer.peer_id, second_peer_stat);
    }

    /// Adds new [`RtcStat`]s for the [`PeerStat`]s from this
    /// [`PeerMetricsService`].
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
                self.metrics_service.do_send(TrafficStopped {
                    source: StoppedMetricSource::PeerTraffic,
                    timestamp: peer_ref.get_stop_time(),
                    peer_id: peer_ref.peer_id,
                    room_id: self.room_id.clone(),
                });
            } else if peer_ref.is_conforms_spec() {
                self.metrics_service.do_send(TrafficFlows {
                    room_id: self.room_id.clone(),
                    peer_id,
                    source: FlowMetricSource::PeerTraffic,
                    timestamp: Instant::now(),
                });
                peer_ref.set_state(PeerStatState::Connected);
                if let Some(partner_peer_id) = peer_ref.get_partner_peer_id() {
                    self.metrics_service.do_send(TrafficFlows {
                        room_id: self.room_id.clone(),
                        peer_id: partner_peer_id,
                        source: FlowMetricSource::PartnerPeerTraffic,
                        timestamp: Instant::now(),
                    });
                }
            } else {
                self.fatal_peer_error(peer_ref.peer_id);
            }
        }
    }

    /// [`Room`] notifies [`PeerMetricsService`] that some [`Peer`] is removed.
    ///
    /// This will be considered as [`TrafficStopped`] for the
    /// [`MetricsCallbackService`].
    pub fn peer_removed(&mut self, peer_id: PeerId) {
        if self.peers.remove(&peer_id).is_some() {
            self.metrics_service.do_send(TrafficStopped {
                peer_id,
                room_id: self.room_id.clone(),
                timestamp: Instant::now(),
                source: StoppedMetricSource::PeerRemoved,
            });
        }
    }
}
