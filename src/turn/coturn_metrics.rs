use crate::{
    api::control::RoomId,
    signalling::metrics_service::{
        FlowMetricSource, MetricsService, StoppedMetricSource, TrafficFlows,
        TrafficStopped,
    },
    turn::coturn_stats::{CoturnAllocationEvent, CoturnEvent},
};
use actix::{
    Actor, Addr, AsyncContext, Handler, Message, StreamHandler, WrapFuture,
};
use futures::{channel::mpsc, StreamExt as _};
use medea_client_api_proto::PeerId;
use patched_redis::ConnectionInfo;
use std::{collections::HashMap, time::Instant};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CoturnUsername {
    pub room_id: RoomId,
    pub peer_id: PeerId,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CoturnPeerStat {
    pub allocations_count: u64,
}

#[derive(Debug)]
pub struct CoturnMetrics {
    metrics_service: Addr<MetricsService>,
    client: patched_redis::Client,
    allocations: HashMap<CoturnUsername, CoturnPeerStat>,
}

impl CoturnMetrics {
    pub fn new(
        cf: &crate::conf::turn::Turn,
        metrics_service: Addr<MetricsService>,
    ) -> Self {
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
            metrics_service,
            client,
            allocations: HashMap::new(),
        }
    }
}

impl Actor for CoturnMetrics {
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

        // TODO: add watchdog

        ctx.add_stream(msg_stream);
    }
}

impl StreamHandler<Option<patched_redis::Msg>> for CoturnMetrics {
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

            let username = CoturnUsername {
                room_id: event.room_id.clone(),
                peer_id: event.peer_id,
            };

            let mut peer_stat =
                self.allocations.entry(username).or_insert(CoturnPeerStat {
                    allocations_count: 0,
                });
            match event.event {
                CoturnAllocationEvent::Traffic { traffic } => {
                    peer_stat.allocations_count += 1;
                    let is_traffic_really_going =
                        traffic.sent_packets + traffic.received_packets > 10;
                    if is_traffic_really_going {
                        self.metrics_service.do_send(TrafficFlows {
                            peer_id: event.peer_id,
                            room_id: event.room_id.clone(),
                            timestamp: Instant::now(),
                            source: FlowMetricSource::Coturn,
                        })
                    }
                }
                CoturnAllocationEvent::Deleted => {
                    peer_stat.allocations_count -= 1;
                    if peer_stat.allocations_count == 0 {
                        self.metrics_service.do_send(TrafficStopped {
                            peer_id: event.peer_id,
                            room_id: event.room_id.clone(),
                            timestamp: Instant::now(),
                            source: StoppedMetricSource::Coturn,
                        });
                    }
                }
                _ => (),
            }
        }
    }
}
