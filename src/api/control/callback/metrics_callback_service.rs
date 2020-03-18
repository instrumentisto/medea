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

#[derive(Debug, Default)]
pub struct MetricsCallbacksService {
    stats: HashMap<RoomId, RoomStats>,
}

impl MetricsCallbacksService {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    pub fn remove_peer(&mut self, room_id: &RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(room_id) {
            room.peers.remove(&peer_id);
        }
    }

    fn fatal_peer_error(&mut self, room_id: &RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(&room_id) {
            room.peers.remove(&peer_id);
            room.room.do_send(PeerSpecContradiction { peer_id });
        }
    }

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

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TrafficFlows {
    pub room_id: RoomId,
    pub peer_id: PeerId,
    pub timestamp: Instant,
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

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TrafficStopped {
    pub room_id: RoomId,
    pub peer_id: PeerId,
    pub timestamp: Instant,
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
                room.room.do_send(PeerStopped(peer.peer_id));
            }
        }
        self.remove_peer(&msg.room_id, msg.peer_id);
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, VariantCount)]
pub enum FlowMetricSource {
    PartnerPeerTraffic,
    PeerTraffic,
    Coturn,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum StoppedMetricSource {
    // TODO: PartnerPeer,
    PeerTraffic,
    Coturn,
    Timeout,
    PeerRemoved,
}

#[derive(Debug)]
pub enum PeerState {
    Started(HashSet<FlowMetricSource>),
    Stopped,
}

#[derive(Debug)]
pub struct PeerStat {
    pub peer_id: PeerId,
    pub state: PeerState,
    pub last_update: Instant,
}

#[derive(Debug)]
pub struct RoomStats {
    room_id: RoomId,
    room: Addr<Room>,
    peers: HashMap<PeerId, PeerStat>,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RegisterRoom {
    pub room_id: RoomId,
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

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct Subscribe {
    pub room_id: RoomId,
    pub peer_id: PeerId,
}

impl Handler<Subscribe> for MetricsCallbacksService {
    type Result = ();

    fn handle(
        &mut self,
        msg: Subscribe,
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

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct UnsubscribePeer {
    pub room_id: RoomId,
    pub peers_ids: HashSet<PeerId>,
}

impl Handler<UnsubscribePeer> for MetricsCallbacksService {
    type Result = ();

    fn handle(
        &mut self,
        msg: UnsubscribePeer,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room_stats) = self.stats.get_mut(&msg.room_id) {
            for peer_id in msg.peers_ids {
                room_stats.peers.remove(&peer_id);
            }
        }
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RemovePeer {
    pub room_id: RoomId,
    pub peer_id: PeerId,
}

impl Handler<RemovePeer> for MetricsCallbacksService {
    type Result = ();

    fn handle(
        &mut self,
        msg: RemovePeer,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.remove_peer(&msg.room_id, msg.peer_id);
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct FatalPeerError {
    pub room_id: RoomId,
    pub peer_id: PeerId,
}

impl Handler<FatalPeerError> for MetricsCallbacksService {
    type Result = ();

    fn handle(
        &mut self,
        msg: FatalPeerError,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.fatal_peer_error(&msg.room_id, msg.peer_id);
    }
}
