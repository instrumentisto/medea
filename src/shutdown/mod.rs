//! A class to handle shutdown signals and to shut down system
//! Actix system has to be running for it to work.

use std::{
    collections::{BTreeMap, HashSet},
    mem,
    time::Duration,
};

use actix::{
    self,
    Addr,
    AsyncContext, Handler, Message, prelude::{Actor, Context}, Recipient, ResponseActFuture, System,
    WrapFuture,
};
use tokio::prelude::{
    future::{self, Future, join_all},
    FutureExt,
    stream::*,
};

use crate::log::prelude::*;

#[derive(Ord, PartialOrd, PartialEq, Eq, Copy, Clone)]
pub struct Priority(pub u8);

#[derive(Debug, Message)]
#[rtype(result = "Result<(),()>")]
#[allow(clippy::module_name_repetitions)]
pub struct ShutdownMessage;

pub struct Subscriber {
    pub priority: Priority,
    pub addr: Recipient<ShutdownMessage>,
}

/// Subscribe to shutdown messages
#[derive(Message)]
#[rtype(result = "()")]
pub struct Subscribe(pub Subscriber);

/// Unsubscribe from shutdown messages
#[derive(Message)]
#[rtype(result = "()")]
pub struct Unsubscribe(pub Subscriber);

/// Send this when a signal is detected
#[cfg(unix)]
#[derive(Message)]
#[rtype(result = "Result<(),()>")]
struct ShutdownSignalDetected(i32);

#[allow(clippy::module_name_repetitions)]
pub struct GracefulShutdown {
    /// [`Actor`]s to send message when graceful shutdown
    recipients: BTreeMap<Priority, HashSet<Recipient<ShutdownMessage>>>,

    /// Timeout after which all [`Actors`] will be forced shutdown
    timeout: Duration,
}

impl GracefulShutdown {
    fn new(timeout: Duration) -> Self {
        Self {
            recipients: BTreeMap::new(),
            timeout,
        }
    }
}

impl Actor for GracefulShutdown {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        #[cfg(not(unix))]
        {
            warning!(
                "Unable to use graceful_shutdown: only UNIX signals are \
                 supported"
            );
            return;
        }
        #[cfg(unix)]
        {
            use tokio_signal::unix::{
                Signal, SIGHUP, SIGINT, SIGQUIT, SIGTERM,
            };

            let mut signals_stream: Box<
                dyn Stream<Error = std::io::Error, Item = i32>,
            > = Box::new(Signal::new(SIGINT).flatten_stream());
            for s in &[SIGHUP, SIGQUIT, SIGTERM] {
                signals_stream = Box::new(
                    signals_stream.select(Signal::new(*s).flatten_stream()),
                );
            }

            ctx.add_message_stream(
                signals_stream.map(ShutdownSignalDetected).map_err(|_| {
                    error!("Error getting shutdown signal");
                }),
            )
        }
    }
}

#[cfg(unix)]
impl Handler<ShutdownSignalDetected> for GracefulShutdown {
    type Result = ResponseActFuture<Self, (), ()>;

    fn handle(
        &mut self,
        _: ShutdownSignalDetected,
        _: &mut Context<Self>,
    ) -> ResponseActFuture<Self, (), ()> {
        info!("Exit signal received, exiting");

        if self.recipients.is_empty() {
            error!("GracefulShutdown: No subscribers registered");
        }

        let mut shutdown_future: Box<dyn Future<Item = Vec<Result<(), ()>>, Error = ()>> =
            Box::new(futures::future::ok(vec![]));

        for recipients_values in self.recipients.values() {
            let mut this_priority_futures_vec =
                Vec::with_capacity(self.recipients.len());

            for recipient in recipients_values.iter() {
                let send_future = recipient.send(ShutdownMessage {});

                let recipient_shutdown_fut = send_future.map_err(|e| {
                    error!("Error sending shutdown message: {:?}", e);
                });

                this_priority_futures_vec.push(recipient_shutdown_fut);
            }

            let this_priority_futures = join_all(this_priority_futures_vec);
            let new_shutdown_future =
                Box::new(shutdown_future.then(|_| this_priority_futures));
            // we need to rewrite shutdown_future, otherwise compiler thinks we
            // moved value
            shutdown_future = Box::new(futures::future::ok(()).map(|_| vec![]));

            mem::replace(&mut shutdown_future, new_shutdown_future);
        }

        Box::new(
            shutdown_future
                .timeout(self.timeout)
                .map_err(|e| {
                    error!(
                        "Error trying to shut down system gracefully: {:?}",
                        e
                    );
                })
                .then(move |_| {
                    System::current().stop();
                    future::ok::<(), ()>(())
                })
                .into_actor(self),
        )
    }
}

impl Handler<Subscribe> for GracefulShutdown {
    type Result = ();

    fn handle(&mut self, msg: Subscribe, _: &mut Context<Self>) {
        let hashset_with_current_priority =
            self.recipients.get_mut(&msg.0.priority);

        if let Some(hashset) = hashset_with_current_priority {
            hashset.insert(msg.0.addr);
        } else {
            self.recipients.insert(msg.0.priority, HashSet::new());
            // unwrap should not panic because we have inserted new empty
            // hashset with the key we are trying to get in the line
            // above /\
            let hashset = self.recipients.get_mut(&msg.0.priority).unwrap();
            hashset.insert(msg.0.addr);
        }
    }
}

impl Handler<Unsubscribe> for GracefulShutdown {
    type Result = ();

    fn handle(&mut self, msg: Unsubscribe, _: &mut Context<Self>) {
        let hashset_with_current_priority =
            self.recipients.get_mut(&msg.0.priority);

        if let Some(hashset) = hashset_with_current_priority {
            hashset.remove(&msg.0.addr);
        } else {
            return;
        }
    }
}

pub fn create(timeout: Duration) -> Addr<GracefulShutdown> {
    GracefulShutdown::new(timeout).start()
}
