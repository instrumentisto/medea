use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use actix::{Actor, Addr, AsyncContext, Handler, Message};
use medea_client_api_proto::PeerId;

use crate::{
    api::control::RoomId,
    signalling::{
        room::{PeerStarted, PeerStopped},
        Room,
    },
};

#[derive(Debug)]
pub struct MetricsService {
    stats: HashMap<RoomId, RoomStats>,
}

impl MetricsService {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    pub fn remove_peer(&mut self, room_id: RoomId, peer_id: PeerId) {
        if let Some(room) = self.stats.get_mut(&room_id) {
            room.tracks.remove(&peer_id);
        }
    }
}

impl Actor for MetricsService {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_secs(10), |this, ctx| {
            for stat in this.stats.values() {
                for track in stat.tracks.values() {
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

impl Handler<TrafficFlows> for MetricsService {
    type Result = ();

    fn handle(
        &mut self,
        msg: TrafficFlows,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.tracks.get_mut(&msg.peer_id) {
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
                                if let Some(room) =
                                    this.stats.get_mut(&msg.room_id)
                                {
                                    if let Some(peer) =
                                        room.tracks.get_mut(&msg.peer_id)
                                    {
                                        if let PeerState::Started(srcs) =
                                            &peer.state
                                        {
                                            // TODO: change it to enum variants
                                            //       count
                                            if srcs.len() < 3 {
                                                // TODO: FATAL ERROR
                                                println!("VALIDATION FAILED {:?}", srcs);
                                            } else {
                                                println!(
                                                    "YAAAAAY VALIDATION PASSED"
                                                );
                                            }
                                        }
                                    }
                                }
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

impl Handler<TrafficStopped> for MetricsService {
    type Result = ();

    fn handle(
        &mut self,
        msg: TrafficStopped,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.tracks.remove(&msg.peer_id) {
                room.room.do_send(PeerStopped(peer.peer_id));
            }
        }
        self.remove_peer(msg.room_id, msg.peer_id);
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum FlowMetricSource {
    PartnerPeer,
    Peer,
    Coturn,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum StoppedMetricSource {
    // TODO: PartnerPeer,
    Peer,
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
    tracks: HashMap<PeerId, PeerStat>,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RegisterRoom {
    pub room_id: RoomId,
    pub room: Addr<Room>,

impl Handler<RegisterRoom> for MetricsService {
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
                tracks: HashMap::new(),
            },
        );
    }
}
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct UnregisterRoom(pub RoomId);

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct AddPeer {
    pub room_id: RoomId,
    pub peer_id: PeerId,
}

impl Handler<AddPeer> for MetricsService {
    type Result = ();

    fn handle(&mut self, msg: AddPeer, _: &mut Self::Context) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            room.tracks.insert(
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
pub struct RemovePeer {
    pub room_id: RoomId,
    pub peer_id: PeerId,
}

impl Handler<RemovePeer> for MetricsService {
    type Result = ();

    fn handle(
        &mut self,
        msg: RemovePeer,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.remove_peer(msg.room_id, msg.peer_id);
    }
}
