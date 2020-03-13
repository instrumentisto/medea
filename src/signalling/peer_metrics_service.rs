use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
    time::Instant,
};

use actix::{Actor, Addr, Handler, Message};
use medea_client_api_proto::{
    stats::{
        RtcInboundRtpStreamMediaType, RtcInboundRtpStreamStats,
        RtcOutboundRtpStreamMediaType, RtcOutboundRtpStreamStats, RtcStatsType,
        StatId,
    },
    MediaType, PeerId,
};

use crate::{
    api::control::RoomId,
    signalling::metrics_service::{
        FlowMetricSource, MetricsService, TrafficFlows,
    },
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
    pub fn update(&mut self, upd: Box<RtcOutboundRtpStreamStats>) {
        self.last_update = Instant::now();
        self.packets_sent = upd.packets_sent;
    }
}

#[derive(Debug)]
struct ReceiveStat {
    last_update: Instant,
    packets_received: u64,
    media_type: TrackMediaType,
}

impl ReceiveStat {
    pub fn update(&mut self, upd: Box<RtcInboundRtpStreamStats>) {
        self.last_update = Instant::now();
        self.packets_received = upd.packets_received;
    }
}

#[derive(Debug)]
struct PeerStat {
    partner_peer: Weak<RefCell<PeerStat>>,
    spec: PeerSpec,
    senders: HashMap<StatId, SenderStat>,
    receivers: HashMap<StatId, ReceiveStat>,
    last_update: Instant,
}

impl PeerStat {
    pub fn update_sender(
        &mut self,
        stat_id: StatId,
        upd: Box<RtcOutboundRtpStreamStats>,
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

    pub fn update_received(
        &mut self,
        stat_id: StatId,
        upd: Box<RtcInboundRtpStreamStats>,
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

    pub fn is_started(&self) -> bool {
        let mut spec_senders: Vec<_> = self.spec.senders.clone();
        let mut spec_receivers: Vec<_> = self.spec.received.clone();
        spec_senders.sort();
        spec_receivers.sort();

        let mut current_senders: Vec<_> = self
            .senders
            .values()
            .map(|sender| sender.media_type)
            .collect();
        let mut current_receivers: Vec<_> = self
            .receivers
            .values()
            .map(|receivers| receivers.media_type)
            .collect();
        current_receivers.sort();
        current_senders.sort();

        spec_receivers == current_receivers && spec_senders == current_senders
    }
}

#[derive(Debug)]
pub struct PeerMetricsService {
    room_id: RoomId,
    metrics_service: Addr<MetricsService>,
    peers: HashMap<PeerId, Rc<RefCell<PeerStat>>>,
}

impl PeerMetricsService {
    pub fn new(room_id: RoomId, metrics_service: Addr<MetricsService>) -> Self {
        Self {
            room_id,
            metrics_service,
            peers: HashMap::new(),
        }
    }
}

impl Actor for PeerMetricsService {
    type Context = actix::Context<Self>;
}

#[derive(Debug)]
pub struct Peer {
    pub peer_id: PeerId,
    pub spec: PeerSpec,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct AddPeers {
    pub first_peer: Peer,
    pub second_peer: Peer,
}

impl Handler<AddPeers> for PeerMetricsService {
    type Result = ();

    fn handle(
        &mut self,
        msg: AddPeers,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let first_peer = Rc::new(RefCell::new(PeerStat {
            partner_peer: Weak::new(),
            last_update: Instant::now(),
            senders: HashMap::new(),
            receivers: HashMap::new(),
            spec: msg.first_peer.spec,
        }));
        let second_peer = Rc::new(RefCell::new(PeerStat {
            partner_peer: Rc::downgrade(&first_peer),
            last_update: Instant::now(),
            senders: HashMap::new(),
            receivers: HashMap::new(),
            spec: msg.second_peer.spec,
        }));
        first_peer.borrow_mut().partner_peer = Rc::downgrade(&second_peer);

        self.peers.insert(msg.first_peer.peer_id, first_peer);
        self.peers.insert(msg.second_peer.peer_id, second_peer);
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct AddStat {
    pub peer_id: PeerId,
    pub stat: Vec<RtcStatsType>,
}

impl Handler<AddStat> for PeerMetricsService {
    type Result = ();

    fn handle(
        &mut self,
        msg: AddStat,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(peer) = self.peers.get(&msg.peer_id) {
            let mut peer_ref = peer.borrow_mut();

            for stat in msg.stat {
                match stat {
                    RtcStatsType::InboundRtp(stat) => {
                        peer_ref.update_received(stat.id, stat.stats);
                    }
                    RtcStatsType::OutboundRtp(stat) => {
                        peer_ref.update_sender(stat.id, stat.stats);
                    }
                    _ => (),
                }
            }

            if peer_ref.is_started() {
                self.metrics_service.do_send(TrafficFlows {
                    room_id: self.room_id.clone(),
                    peer_id: msg.peer_id,
                    source: FlowMetricSource::Peer,
                    timestamp: Instant::now(),
                });
            }
        }
    }
}
