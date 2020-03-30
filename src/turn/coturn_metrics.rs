//! Service which is responsible for processing [`PeerConnection`]'s metrics
//! received from the Coturn.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use actix::{
    fut::Either, Actor, ActorFuture, Addr, AsyncContext, StreamHandler,
    WrapFuture,
};
use futures::{channel::mpsc, StreamExt as _};
use redis_pub_sub::ConnectionInfo;

use crate::{
    log::prelude::*,
    signalling::peers_traffic_watcher::{
        FlowMetricSource, PeersTrafficWatcher, StoppedMetricSource,
        TrafficFlows, TrafficStopped,
    },
};

use super::{
    allocation_event::{CoturnAllocationEvent, CoturnEvent},
    CoturnUsername,
};

/// Channel pattern used to subscribe to all allocation events published by
/// Coturn.
const ALLOCATIONS_CHANNEL_PATTERN: &str = "turn/realm/*/user/*/allocation/*";

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<O> =
    Box<dyn ActorFuture<Actor = CoturnMetricsService, Output = O>>;

/// Service which is responsible for processing [`PeerConnection`]'s metrics
/// received from the Coturn.
#[derive(Debug)]
pub struct CoturnMetricsService {
    /// [`Addr`] of [`MetricsCallbackService`] to which traffic updates will be
    /// sent.
    metrics_service: Addr<PeersTrafficWatcher>,

    /// Redis client with which Coturn stat updates will be received.
    client: redis_pub_sub::Client,

    /// Count of allocations for the [`CoturnUsername`] (which acts as a key).
    allocations_count: HashMap<CoturnUsername, u64>,
}

impl CoturnMetricsService {
    /// Returns new [`CoturnMetricsService`] service.
    ///
    /// # Errors
    ///
    /// [`RedisError`] can be returned if some basic check on the URL is failed.
    pub fn new(
        cf: &crate::conf::turn::Turn,
        metrics_service: Addr<PeersTrafficWatcher>,
    ) -> Result<Self, redis_pub_sub::RedisError> {
        let connection_info = ConnectionInfo {
            addr: Box::new(redis_pub_sub::ConnectionAddr::Tcp(
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
        let client = redis_pub_sub::Client::open(connection_info)?;

        Ok(Self {
            client,
            allocations_count: HashMap::new(),
            metrics_service,
        })
    }

    /// Opens new Redis connection, subscribes to the Coturn events and adds
    /// [`Stream`] with this events to this the [`CoturnMetricsService`]'s
    /// context.
    fn connect_and_subscribe(
        &mut self,
    ) -> ActFuture<Result<(), redis_pub_sub::RedisError>> {
        let (msg_tx, msg_stream) = mpsc::unbounded();
        let client = self.client.clone();

        Box::new(
            async move {
                let conn = client.get_async_connection().await?;
                let mut pubsub = conn.into_pubsub();
                pubsub.psubscribe(ALLOCATIONS_CHANNEL_PATTERN).await?;

                Ok(pubsub)
            }
            .into_actor(self)
            .map(
                |res: Result<_, redis_pub_sub::RedisError>, this, ctx| {
                    let mut pubsub = res?;
                    ctx.spawn(
                        async move {
                            let mut msg_stream = pubsub.on_message();
                            while let Some(msg) = msg_stream.next().await {
                                if msg_tx.unbounded_send(msg).is_err() {
                                    break;
                                }
                            }
                        }
                        .into_actor(this),
                    );
                    ctx.add_stream(msg_stream);

                    Ok(())
                },
            ),
        )
    }

    /// Connects Redis until success.
    fn connect_until_success(&mut self) -> ActFuture<()> {
        Box::new(self.connect_and_subscribe().then(|res, this, _| {
            if let Err(err) = res {
                warn!(
                    "Error while creating Redis PubSub connection for the \
                     CoturnMetricsService: {:?}",
                    err
                );

                Either::Left(
                    tokio::time::delay_for(Duration::from_secs(1))
                        .into_actor(this)
                        .then(|_, this, _| this.connect_until_success()),
                )
            } else {
                Either::Right(async {}.into_actor(this))
            }
        }))
    }
}

impl Actor for CoturnMetricsService {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.wait(self.connect_until_success());
    }
}

impl StreamHandler<redis_pub_sub::Msg> for CoturnMetricsService {
    fn handle(&mut self, msg: redis_pub_sub::Msg, _: &mut Self::Context) {
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
        ctx.wait(self.connect_until_success());
    }
}
