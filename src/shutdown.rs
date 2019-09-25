//! Graceful shutdown implementation.

use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

#[cfg(unix)]
use actix::AsyncContext;
use actix::{
    prelude::{Actor, Context},
    Addr, Handler, Message, Recipient, ResponseFuture, System,
};
use derive_more::Display;
use failure::Fail;
use futures::{future, stream::iter_ok, Future, Stream};
use tokio::util::FutureExt as _;

use crate::log::prelude::*;

/// Priority that [`Subscriber`] should be triggered to shutdown gracefully
/// with.
#[derive(Clone, Copy, Eq, Ord, PartialOrd, PartialEq)]
pub struct Priority(pub u8);

/// Message that [`Subscriber`] is informed with to perform its graceful
/// shutdown.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct ShutdownGracefully;

/// Service which listens incoming OS signals and performs graceful
/// shutdown for all its [`Subscriber`]s.
#[allow(clippy::module_name_repetitions)]
pub struct GracefulShutdown {
    /// Subscribers being subscribed to [`GracefulShutdown`] service.
    subs: BTreeMap<Priority, HashSet<Recipient<ShutdownGracefully>>>,
    /// Timeout for shutdown to complete gracefully.
    timeout: Duration,
    /// Current state of [`GracefulShutdown`] service.
    state: State,
}

/// Possible state of [`GracefulShutdown`] service.
enum State {
    /// Service is up and listening to OS signals.
    Listening,
    /// Service is performing graceful shutdown at the moment.
    InProgress,
}

impl GracefulShutdown {
    /// Creates new [`GracefulShutdown`] service.
    #[inline]
    pub fn new(timeout: Duration) -> Self {
        Self {
            subs: BTreeMap::new(),
            timeout,
            state: State::Listening,
        }
    }
}

impl Actor for GracefulShutdown {
    type Context = Context<Self>;

    #[cfg(not(unix))]
    fn started(&mut self, _: &mut Self::Context) {
        warn!(
            "Graceful shutdown is disabled: only UNIX signals are supported, \
             and current playform is not UNIX"
        );
    }

    #[cfg(unix)]
    fn started(&mut self, ctx: &mut Self::Context) {
        use tokio_signal::unix::{Signal, SIGHUP, SIGINT, SIGQUIT, SIGTERM};
        for s in &[SIGHUP, SIGINT, SIGQUIT, SIGTERM] {
            ctx.add_message_stream(
                Signal::new(*s)
                    .flatten_stream()
                    .map(OsSignal)
                    .map_err(|_| error!("Error getting shutdown signal")),
            );
        }
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        if let State::Listening = self.state {
            info!("Graceful shutdown has been completed");
        }
    }
}

/// Message that is received by [`GracefulShutdown`] shutdown service when
/// the process receives an OS signal.
#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
struct OsSignal(i32);

impl Handler<OsSignal> for GracefulShutdown {
    type Result = ResponseFuture<(), ()>;

    fn handle(&mut self, sig: OsSignal, _: &mut Context<Self>) -> Self::Result {
        info!("OS signal '{}' received", sig.0);

        match self.state {
            State::InProgress => {
                return Box::new(future::ok(()));
            }
            State::Listening => {
                self.state = State::InProgress;
            }
        }

        info!("Initiating graceful shutdown...");

        if self.subs.is_empty() {
            System::current().stop();
            return Box::new(future::ok(()));
        }

        let by_priority: Vec<_> = self
            .subs
            .values()
            .rev()
            .map(|addrs| {
                let addrs: Vec<_> = addrs
                    .iter()
                    .map(|addr| {
                        addr.send(ShutdownGracefully).map(|_| ()).or_else(|e| {
                            error!("Error requesting shutdown: {}", e);
                            future::ok::<(), ()>(())
                        })
                    })
                    .collect();
                future::join_all(addrs)
            })
            .collect();

        Box::new(
            iter_ok::<_, ()>(by_priority)
                .for_each(|row| row.map(|_| ()))
                .timeout(self.timeout)
                .map_err(|_| {
                    error!("Graceful shutdown has timed out, stopping system");
                    System::current().stop()
                })
                .map(|_| {
                    info!("Graceful shutdown succeeded, stopping system");
                    System::current().stop()
                }),
        )
    }
}

/// Subscriber to [`GracefulShutdown`] service, which is notified when
/// graceful shutdown happens.
pub struct Subscriber {
    /// Priority that [`Subscriber`] should be notified with.
    ///
    /// Higher priority means that [`Subscriber`] will be notified sooner.
    /// [`Subscriber`] won't be notified until all other [`Subscriber`]s with
    /// higher priority will complete their shutdown.
    pub priority: Priority,

    /// Address of [`Subscriber`] to inform it about graceful shutdown via.
    pub addr: Recipient<ShutdownGracefully>,
}

/// Message that [`Subscriber`] subscribes to shutdown messages with.
#[derive(Message)]
#[rtype(result = "Result<(), ShuttingDownError>")]
struct Subscribe(pub Subscriber);

impl Handler<Subscribe> for GracefulShutdown {
    type Result = Result<(), ShuttingDownError>;

    /// Subscribes provided [`Subscriber`] to shutdown notifications.
    ///
    /// Returns [`ShuttingDownError`] if shutdown happens at the moment.
    fn handle(&mut self, m: Subscribe, _: &mut Context<Self>) -> Self::Result {
        if let State::InProgress = self.state {
            return Err(ShuttingDownError);
        }
        let addrs = self.subs.entry(m.0.priority).or_insert_with(HashSet::new);
        addrs.insert(m.0.addr);
        Ok(())
    }
}

/// Error which indicates that process is shutting down at this moment.
#[derive(Clone, Copy, Debug, Display, Fail)]
#[display(fmt = "Process is shutting down at the moment")]
pub struct ShuttingDownError;

/// Message that [`Subscriber`] unsubscribes from receiving shutdown
/// notifications with.
#[derive(Message)]
#[rtype(result = "()")]
struct Unsubscribe(pub Subscriber);

impl Handler<Unsubscribe> for GracefulShutdown {
    type Result = ();

    /// Unsubscribes provided [`Subscriber`] to shutdown notifications.
    fn handle(&mut self, m: Unsubscribe, _: &mut Context<Self>) {
        let mut remove = false;
        if let Some(addrs) = self.subs.get_mut(&m.0.priority) {
            addrs.remove(&m.0.addr);
            if addrs.is_empty() {
                remove = true;
            }
        }
        if remove {
            self.subs.remove(&m.0.priority);
        }
    }
}

/// Subscribes recipient to [`GracefulShutdown`].
pub fn subscribe(
    shutdown_addr: &Addr<GracefulShutdown>,
    subscriber: Recipient<ShutdownGracefully>,
    priority: Priority,
) {
    shutdown_addr.do_send(Subscribe(Subscriber {
        priority,
        addr: subscriber,
    }));
}

/// Unsubscribes recipient from [`GracefulShutdown`].
pub fn unsubscribe(
    shutdown_addr: &Addr<GracefulShutdown>,
    subscriber: Recipient<ShutdownGracefully>,
    priority: Priority,
) {
    shutdown_addr.do_send(Unsubscribe(Subscriber {
        priority,
        addr: subscriber,
    }));
}
