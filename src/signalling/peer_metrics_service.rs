use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use actix::Addr;
use medea_client_api_proto::{
    stats::{
        RtcInboundRtpStreamMediaType, RtcInboundRtpStreamStats,
        RtcOutboundRtpStreamMediaType, RtcOutboundRtpStreamStats, RtcStat,
        RtcStatsType, StatId,
    },
    PeerId,
};

use crate::api::control::{
    callback::metrics_callback_service::{
        FatalPeerError, FlowMetricSource, MetricsCallbacksService,
        StoppedMetricSource, TrafficFlows, TrafficStopped,
    },
    RoomId,
};

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

#[derive(Debug)]
pub struct PeerSpec {
    pub senders: Vec<TrackMediaType>,
    pub received: Vec<TrackMediaType>,
}

#[derive(Debug)]
struct SenderStat {
    last_update: Instant,
    packets_sent: u64,
    media_type: TrackMediaType,
}

impl SenderStat {
    fn update(&mut self, upd: &RtcOutboundRtpStreamStats) {
        self.last_update = Instant::now();
        self.packets_sent = upd.packets_sent;
    }

    fn is_active(&self) -> bool {
        self.last_update > Instant::now() - Duration::from_secs(10)
    }
}

#[derive(Debug)]
struct ReceiveStat {
    last_update: Instant,
    packets_received: u64,
    media_type: TrackMediaType,
}

impl ReceiveStat {
    fn update(&mut self, upd: &RtcInboundRtpStreamStats) {
        self.last_update = Instant::now();
        self.packets_received = upd.packets_received;
    }

    fn is_active(&self) -> bool {
        self.last_update > Instant::now() - Duration::from_secs(10)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeerStatState {
    Connected,
    Waiting,
}

#[derive(Debug)]
struct PeerStat {
    peer_id: PeerId,
    partner_peer: Weak<RefCell<PeerStat>>,
    spec: PeerSpec,
    senders: HashMap<StatId, SenderStat>,
    receivers: HashMap<StatId, ReceiveStat>,
    state: PeerStatState,
    last_update: Instant,
}

impl PeerStat {
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

    fn update_received(
        &mut self,
        stat_id: StatId,
        upd: &RtcInboundRtpStreamStats,
    ) {
        self.receivers
            .entry(stat_id)
            .or_insert_with(|| ReceiveStat {
                last_update: Instant::now(),
                packets_received: 0,
                media_type: TrackMediaType::from(&upd.media_specific_stats),
            })
            .update(upd);
    }

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

    fn get_stop_time(&self) -> Instant {
        self.senders
            .values()
            .map(|send| send.last_update)
            .chain(self.receivers.values().map(|recv| recv.last_update))
            .max()
            .unwrap_or_else(Instant::now)
    }

    fn get_partner_peer_id(&self) -> Option<PeerId> {
        self.partner_peer
            .upgrade()
            .map(|partner_peer| partner_peer.borrow().get_peer_id())
    }

    fn get_peer_id(&self) -> PeerId {
        self.peer_id
    }

    fn set_state(&mut self, state: PeerStatState) {
        self.state = state;
    }
}

#[derive(Debug)]
pub struct Peer {
    pub peer_id: PeerId,
    pub spec: PeerSpec,
}

#[derive(Debug)]
pub struct PeerMetricsService {
    room_id: RoomId,
    metrics_service: Addr<MetricsCallbacksService>,
    peers: HashMap<PeerId, Rc<RefCell<PeerStat>>>,
}

impl PeerMetricsService {
    pub fn new(
        room_id: RoomId,
        metrics_service: Addr<MetricsCallbacksService>,
    ) -> Self {
        Self {
            room_id,
            metrics_service,
            peers: HashMap::new(),
        }
    }

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
                self.metrics_service.do_send(FatalPeerError {
                    room_id: self.room_id.clone(),
                    peer_id: peer_ref.peer_id,
                });
            }
        }

        println!("Peers was validated!");
    }

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

    pub fn add_stat(&mut self, peer_id: PeerId, stats: Vec<RtcStat>) {
        if let Some(peer) = self.peers.get(&peer_id) {
            let mut peer_ref = peer.borrow_mut();

            for stat in stats {
                match &stat.stats {
                    RtcStatsType::InboundRtp(inbound) => {
                        peer_ref.update_received(stat.id, inbound);
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
                self.metrics_service.do_send(FatalPeerError {
                    room_id: self.room_id.clone(),
                    peer_id: peer_ref.peer_id,
                });
            }
        }
    }

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
