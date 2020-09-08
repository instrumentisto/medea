//! Service responsible for processing [`PeerConnection`]'s metrics received
//! from Coturn.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use actix::{
    fut::Either, Actor, ActorFuture, AsyncContext, StreamHandler, WrapFuture,
};
use futures::{channel::mpsc, StreamExt as _};
use redis::{ConnectionInfo, RedisError};

use crate::{
    log::prelude::*,
    signalling::peers::{FlowMetricSource, PeerTrafficWatcher},
};

use super::{
    allocation_event::{CoturnAllocationEvent, CoturnEvent},
    CoturnUsername,
};

/// Channel pattern used to subscribe to all allocation events published by
/// Coturn.
const ALLOCATIONS_CHANNEL_PATTERN: &str = "turn/realm/*/user/*/allocation/*";

/// Ergonomic type alias for using [`ActorFuture`] by [`CoturnMetricsService`].
pub type ActFuture<O> =
    Box<dyn ActorFuture<Actor = CoturnMetricsService, Output = O>>;

/// Service responsible for processing [`PeerConnection`]'s metrics received
/// from Coturn.
#[derive(Debug)]
pub struct CoturnMetricsService {
    /// [`PeerTrafficWatcher`] which will be notified of all traffic events.
    peer_traffic_watcher: Arc<dyn PeerTrafficWatcher>,

    /// Redis client with which Coturn stat updates are received.
    client: redis::Client,

    /// Count of allocations for each [`CoturnUsername`] (which acts as a key).
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
        peer_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
    ) -> Result<Self, RedisError> {
        let client = redis::Client::open(ConnectionInfo::from(&cf.db.redis))?;

        Ok(Self {
            client,
            allocations_count: HashMap::new(),
            peer_traffic_watcher,
        })
    }

    /// Opens new Redis connection, subscribes to Coturn events and injects
    /// [`Stream`] with these events into the [`CoturnMetricsService`]'s
    /// context.
    fn connect_and_subscribe(&mut self) -> ActFuture<Result<(), RedisError>> {
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
            .map(|res: Result<_, RedisError>, this, ctx| {
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
            }),
        )
    }

    /// Connects Redis until succeeds.
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

impl StreamHandler<redis::Msg> for CoturnMetricsService {
    fn handle(&mut self, msg: redis::Msg, _: &mut Self::Context) {
        let event = match CoturnEvent::parse(&msg) {
            Ok(ev) => ev,
            Err(e) => {
                error!("Error parsing CoturnEvent: {}", e);
                return;
            }
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
                    self.peer_traffic_watcher.traffic_flows(
                        event.room_id,
                        event.peer_id,
                        FlowMetricSource::Coturn,
                    )
                }
            }
            CoturnAllocationEvent::Deleted => {
                *allocations_count -= 1;
                if *allocations_count == 0 {
                    self.peer_traffic_watcher.traffic_stopped(
                        event.room_id,
                        event.peer_id,
                        Instant::now(),
                    );
                }
            }
            _ => (),
        }
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        ctx.wait(self.connect_until_success());
    }
}

// TODO: tests: add stream, send different stuff, see what is send to
//       peers_traffic_watcher
