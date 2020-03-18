//! Service which is responsible for processing [`PeerConnection`]'s metrics
//! received from the Coturn.

use std::{collections::HashMap, time::Instant};

use actix::{Actor, Addr, AsyncContext, StreamHandler, WrapFuture};
use futures::{channel::mpsc, StreamExt as _};
use medea_client_api_proto::PeerId;
use patched_redis::ConnectionInfo;

use crate::{
    api::control::{
        callback::metrics_callback_service::{
            FlowMetricSource, MetricsCallbacksService, StoppedMetricSource,
            TrafficFlows, TrafficStopped,
        },
        RoomId,
    },
    turn::allocation_event::{CoturnAllocationEvent, CoturnEvent},
};

/// Username of the Coturn user.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CoturnUsername {
    /// [`RoomId`] of [`Room`] for which this Coturn user is created.
    pub room_id: RoomId,

    /// [`PeerId`] of [`PeerConnection`] for which this Coturn user is created.
    pub peer_id: PeerId,
}

/// Service which is responsible for processing [`PeerConnection`]'s metrics
/// received from the Coturn.
#[derive(Debug)]
pub struct CoturnMetrics {
    /// [`Addr`] of [`MetricsCallbackService`] to which traffic updates will be
    /// sent.
    metrics_service: Addr<MetricsCallbacksService>,

    /// Redis client with which Coturn stat updates will be received.
    client: patched_redis::Client,

    /// Count of allocations for the [`CoturnUsername`] (which acts as a key).
    allocations_count: HashMap<CoturnUsername, u64>,
}

impl CoturnMetrics {
    /// Returns new [`CoturnMetrics`] service.
    pub fn new(
        cf: &crate::conf::turn::Turn,
        metrics_service: Addr<MetricsCallbacksService>,
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
            allocations_count: HashMap::new(),
        }
    }

    fn add_redis_stream(&mut self, ctx: &mut <Self as Actor>::Context) {
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
                while let Some(msg) = msg_stream.next().await {
                    if msg_tx.unbounded_send(msg).is_err() {
                        break;
                    }
                }
            }
            .into_actor(self),
        );

        ctx.add_stream(msg_stream);
    }
}

impl Actor for CoturnMetrics {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.add_redis_stream(ctx);
    }
}

impl StreamHandler<patched_redis::Msg> for CoturnMetrics {
    fn handle(&mut self, msg: patched_redis::Msg, _: &mut Self::Context) {
        let event = if let Ok(event) = CoturnEvent::parse(&msg) {
            event
        } else {
            return;
        };

        let username = CoturnUsername {
            room_id: event.room_id.clone(),
            peer_id: event.peer_id,
        };

        let allocations_count =
            self.allocations_count.entry(username).or_insert(0);
        match event.event {
            CoturnAllocationEvent::Traffic { traffic } => {
                *allocations_count += 1;
                let is_traffic_really_going =
                    traffic.sent_packets + traffic.received_packets > 10;
                if is_traffic_really_going {
                    self.metrics_service.do_send(TrafficFlows {
                        peer_id: event.peer_id,
                        room_id: event.room_id,
                        timestamp: Instant::now(),
                        source: FlowMetricSource::Coturn,
                    })
                }
            }
            CoturnAllocationEvent::Deleted => {
                *allocations_count -= 1;
                if *allocations_count == 0 {
                    self.metrics_service.do_send(TrafficStopped {
                        peer_id: event.peer_id,
                        room_id: event.room_id,
                        timestamp: Instant::now(),
                        source: StoppedMetricSource::Coturn,
                    });
                }
            }
            _ => (),
        }
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        self.add_redis_stream(ctx);
    }
}
