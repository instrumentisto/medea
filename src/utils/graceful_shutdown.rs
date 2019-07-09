//! A class to handle shutdown signals and to shut down system
//! Actix system has to be running for it to work.

use std::{collections::{BTreeMap, HashSet},
          mem, sync::Mutex, thread,
          time::{Duration, Instant}, sync::mpsc::channel,
          hash::{Hash, Hasher}};

use actix::{self, MailboxError, Message, Recipient, System,
            Handler, WrapFuture, AsyncContext, Addr, Arbiter};
use actix::prelude::{Actor, Context};
use tokio::prelude::{
    future::{self, join_all, Future},
    stream::*,
};
use tokio::timer::Delay;
use futures::future::IntoFuture;
use tokio::prelude::FutureExt;

use lazy_static::lazy_static;

use crate::log::prelude::*;
use tokio::runtime::Runtime;
pub type ShutdownMessageResult = Result<
    Box<dyn Future<Item = (), Error = ()> + std::marker::Send>, ()
>;

type ShutdownFutureType =
    dyn Future<
        Item = Vec<
            Result<Box<dyn futures::future::Future<Error = (), Item = ()> + std::marker::Send>, ()>>,
        Error = ()
    >;

type ShutdownSignalDetectedResult = Result<
    Box<dyn Future<
        Item = (),
        Error = ()
    >>,
()>;

#[derive(Debug)]
pub struct ShutdownMessage;

impl Message for ShutdownMessage {
    type Result = ShutdownMessageResult;
}

/// Subscribe to exit events, with priority
pub struct ShutdownSubscribe {
    pub priority: u8,
    pub who: Recipient<ShutdownMessage>,
}

impl Message for ShutdownSubscribe {
    type Result = ();
}

//todo #[derive(Message)]
/// Subscribe to exit events, with priority
pub struct ShutdownUnsubscribe {
    pub priority: u8,
    pub who: Recipient<ShutdownMessage>,
}

impl Message for ShutdownUnsubscribe {
    type Result = ();
}

/// Send this when a signal is detected
#[cfg(unix)]
struct ShutdownSignalDetected(i32);

#[cfg(unix)]
impl Message for ShutdownSignalDetected {
    type Result = ShutdownSignalDetectedResult;
}


pub struct GracefulShutdown {
    /// [`Actor`]s to send message when graceful shutdown
    recipients: BTreeMap<u8, HashSet<Recipient<ShutdownMessage>>>,

    /// Timeout after which all [`Actors`] will be forced shutdown
    shutdown_timeout: u64,

    //ask
    /// Timeout after which all [`Actors`] will be forced shutdown
    system: actix::System,
}

impl GracefulShutdown {
    fn new(shutdown_timeout: u64, system: actix::System) -> Self {
        Self {
            recipients: BTreeMap::new(),
            shutdown_timeout,
            system
        }
    }
}

impl Actor for GracefulShutdown {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        #[cfg(not(unix))]
            {
                error!("Unable to use graceful_shutdown: only UNIX signals are supported");
                return;
            }
        #[cfg(unix)]
            {
                use tokio_signal::unix::{Signal, SIGHUP, SIGINT, SIGQUIT, SIGTERM};

                let sigint_stream = Signal::new(SIGINT).flatten_stream();
                let sigterm_stream = Signal::new(SIGTERM).flatten_stream();
                let sigquit_stream = Signal::new(SIGQUIT).flatten_stream();
                let sighup_stream = Signal::new(SIGHUP).flatten_stream();
                let signals_stream = sigint_stream
                    .select(sigterm_stream)
                    .select(sigquit_stream)
                    .select(sighup_stream);

                ctx.add_message_stream(signals_stream
                    .map(move |signal| {
                        ShutdownSignalDetected(signal)
                    })
                    .map_err(|_| ())
                );

            }
    }
}

#[cfg(unix)]
impl Handler<ShutdownSignalDetected> for GracefulShutdown {
    type Result = ShutdownSignalDetectedResult;

    fn handle(&mut self, msg: ShutdownSignalDetected, ctx: &mut Context<Self>)
            -> Self::Result {
        use tokio_signal::unix::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};

        match msg.0 {
            SIGINT=> {
                error!("SIGINT received, exiting");
            },
            SIGHUP => {
                error!("SIGHUP received, reloading");
            },
            SIGTERM => {
                error!("SIGTERM received, stopping");
            },
            SIGQUIT => {
                error!("SIGQUIT received, exiting");
            },
            _ => {
                error!("Exit signal received, exiting");
            }
        };

        if self.recipients.is_empty() {
            error!("GracefulShutdown: No subscribers registered");
        }

        let mut shutdown_future: Box<ShutdownFutureType> =
            Box::new(
                futures::future::ok(vec![])
            );

        for recipients_values in self.recipients.values() {
            let mut this_priority_futures_vec =
                Vec::with_capacity(self.recipients.len());

            for recipient in recipients_values.iter() {
                let send_future = recipient.send(ShutdownMessage {});
                //todo async handle

                let recipient_shutdown_fut = send_future
                        .into_future()
                        .map_err(move |e| {
                            error!("Error sending shutdown message: {:?}", e);
                        })
                        .then(|future| {
                            future
                        });

                this_priority_futures_vec.push(recipient_shutdown_fut);
            }

            let this_priority_futures = join_all(this_priority_futures_vec);
            let new_shutdown_future =
                Box::new(shutdown_future.then(|_| this_priority_futures));
            // we need to rewrite shutdown_future, otherwise compiler thinks we
            // moved value
            shutdown_future = Box::new(
                futures::future::ok(())
                    .map(|_| {
                        vec![]
                    })
            );

            mem::replace(&mut shutdown_future, new_shutdown_future);
        }

        let system_to_stop = self.system.clone();
        Ok(Box::new(
            shutdown_future
                .timeout(Duration::from_millis(self.shutdown_timeout))
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
        ))
    }
}

impl Handler<ShutdownSubscribe> for GracefulShutdown {
    type Result = ();

    fn handle(&mut self, msg: ShutdownSubscribe, _: &mut Context<Self>) {
        //ask
        //todo replace vec to hashset

        let hashset_with_current_priority = self.recipients.get_mut(&msg.priority);

        if let Some(hashset) = hashset_with_current_priority {
            hashset.insert(msg.who);
        } else {
            self.recipients.insert(msg.priority, HashSet::new());
            // unwrap should not panic because we have inserted new empty hashset
            // with the key we are trying to get in the line above /\
            let hashset = self.recipients.get_mut(&msg.priority).unwrap();
            hashset.insert(msg.who);
        }
    }
}

impl Handler<ShutdownUnsubscribe> for GracefulShutdown {
    type Result = ();

    fn handle(&mut self, msg: ShutdownUnsubscribe, _: &mut Context<Self>) {
        let hashset_with_current_priority = self.recipients.get_mut(&msg.priority);

        if let Some(hashset) = hashset_with_current_priority {
            hashset.remove(&msg.who);
        } else {
            return;
        }
    }
}


pub fn create(shutdown_timeout: u64, system: actix::System) -> Addr<GracefulShutdown> {
    let graceful_shutdown = GracefulShutdown::new(shutdown_timeout, system).start();
    graceful_shutdown
}
