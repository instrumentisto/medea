use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use actix::{
    Actor, Addr, AsyncContext, Handler, Message, StreamHandler, WrapFuture,
};
use medea_client_api_proto::{stats::StatId, PeerId};

use crate::{
    api::control::RoomId, signalling::Room, turn::coturn_stats::CoturnEvent,
};
use patched_redis::{ConnectionInfo, Msg};

#[derive(Debug)]
pub struct MetricsService {
    stats: HashMap<RoomId, RoomStats>,
    client: patched_redis::Client,
}

impl MetricsService {
    pub fn new(cf: &crate::conf::turn::Turn) -> Self {
        let connection_info = ConnectionInfo {
            addr: Box::new(patched_redis::ConnectionAddr::Tcp(
                cf.db.redis.host.to_string(),
                cf.db.redis.port,
            )),
            db: cf.db.redis.db_number,
            passwd: if cf.db.redis.pass.is_empty() {
                None
            } else {
                Some(cf.db.redis.pass.to_string())
            },
        };
        // TODO: UNWRAP
        let client = patched_redis::Client::open(connection_info).unwrap();

        Self {
            stats: HashMap::new(),
            client,
        }
    }
}

use futures::StreamExt;

impl Actor for MetricsService {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let (msg_tx, msg_stream) = mpsc::unbounded();
        let client = self.client.clone();

        ctx.spawn(
            async move {
                let conn = client.get_async_connection().await.unwrap();
                let mut pubsub = conn.into_pubsub();
                pubsub
                    .psubscribe("turn/realm/*/user/*/allocation/*")
                    .await
                    .unwrap();

                let mut msg_stream = pubsub.on_message();
                while msg_tx.unbounded_send(msg_stream.next().await).is_ok() {}
            }
            .into_actor(self),
        );

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

        ctx.add_stream(msg_stream);
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
                    PeerState::Stopped(_) => {
                        let mut srcs = HashSet::new();
                        srcs.insert(msg.source);
                        peer.state = PeerState::Started(srcs);

                        ctx.run_later(
                            Duration::from_secs(15),
                            move |this, ctx| {
                                if let Some(room) =
                                    this.stats.get_mut(&msg.room_id)
                                {
                                    if let Some(peer) =
                                        room.tracks.get_mut(&msg.peer_id)
                                    {
                                        if let PeerState::Started(srcs) =
                                            &peer.state
                                        {
                                            // TODO: change it to enum variants count
                                            if srcs.len() < 2 {
                                                panic!(
                                                    "\n\n\n\n\n\n\n\n\n\n\n\\
                                                     nVALIDATION \
                                                     FAILED\n\n\n\n\n\\n\n\n\\
                                                     n\n\n\n\n\n\n\n\n\n\n"
                                                )
                                            // TODO: FATAL ERROR
                                            } else {
                                                println!(
                                                    "\n\n\nYAAAAAY VALIDATION \
                                                     PASSED\n\n\n"
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
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            if let Some(peer) = room.tracks.get_mut(&msg.peer_id) {
                peer.last_update = msg.timestamp;
                match &mut peer.state {
                    PeerState::Stopped(sources) => {
                        sources.insert(msg.source);
                    }
                    PeerState::Started(_) => {
                        let mut srcs = HashSet::new();
                        srcs.insert(msg.source);
                        peer.state = PeerState::Stopped(srcs);

                        ctx.run_later(
                            Duration::from_secs(15),
                            move |this, ctx| {
                                if let Some(room) =
                                    this.stats.get_mut(&msg.room_id)
                                {
                                    if let Some(peer) =
                                        room.tracks.get_mut(&msg.peer_id)
                                    {
                                        if let PeerState::Stopped(srcs) =
                                            &peer.state
                                        {
                                            // TODO: change it to enum variants count
                                            if srcs.len() < 3 {
                                                // TODO: FATAL ERROR
                                            }
                                        }
                                    }
                                }
                            },
                        );

                        // TODO: send OnStop.
                    }
                }
            }
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum FlowMetricSource {
    // TODO: PartnerPeer,
    Peer,
    Coturn,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum StoppedMetricSource {
    // TODO: PartnerPeer,
    Peer,
    Coturn,
    Timeout,
}

#[derive(Debug)]
pub enum PeerState {
    Started(HashSet<FlowMetricSource>),
    Stopped(HashSet<StoppedMetricSource>),
}

#[derive(Debug)]
pub struct PeerStat {
    pub peer_id: PeerId,
    pub state: PeerState,
    pub allocations: HashSet<u64>,
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

    fn handle(
        &mut self,
        msg: AddPeer,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.stats.get_mut(&msg.room_id) {
            room.tracks.insert(
                msg.peer_id,
                PeerStat {
                    peer_id: msg.peer_id,
                    state: PeerState::Stopped(HashSet::new()),
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

impl Handler<RegisterRoom> for MetricsService {
    type Result = ();

    fn handle(
        &mut self,
        msg: RegisterRoom,
        ctx: &mut Self::Context,
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

use crate::{
    signalling::room::PeerStarted, turn::coturn_stats::CoturnAllocationEvent,
};
use futures::channel::mpsc;
use medea_client_api_proto::PeerMetrics::PeerConnectionStateChanged;
use std::time::Duration;

impl StreamHandler<Option<patched_redis::Msg>> for MetricsService {
    fn handle(
        &mut self,
        item: Option<patched_redis::Msg>,
        ctx: &mut Self::Context,
    ) {
        if let Some(msg) = item {
            let event = if let Ok(event) = CoturnEvent::parse(&msg) {
                event
            } else {
                return;
            };

            if let Some(room) = self.stats.get(&event.room_id) {
                if room.tracks.contains_key(&event.peer_id) {
                    match event.event {
                        CoturnAllocationEvent::Traffic { traffic } => {
                            if traffic.sent_packets + traffic.received_packets
                                > 10
                            {
                                ctx.notify(TrafficFlows {
                                    peer_id: event.peer_id,
                                    room_id: event.room_id.clone(),
                                    timestamp: Instant::now(),
                                    source: FlowMetricSource::Coturn,
                                });
                            }
                        }
                        CoturnAllocationEvent::Deleted => {
                            //                            
                            // ctx.notify(TrafficStopped {
                            //                                source:
                            // StoppedMetricSource::Coturn,
                            //                                timestamp:
                            // Instant::now(),
                            //                                peer_id:
                            // event.peer_id,
                            //                                room_id:
                            // event.room_id.clone(),
                            //                            })
                        }
                        _ => (),
                    }
                }
            }
        }
    }
}
