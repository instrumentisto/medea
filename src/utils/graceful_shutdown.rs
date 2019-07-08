//! A class to handle shutdown signals and to shut down system
//! Actix system has to be running for it to work.

use std::{collections::BTreeMap, mem, sync::Mutex, thread, time::Duration,
          sync::mpsc::channel};

use actix::{self, MailboxError, Message, Recipient, System,
            Handler, WrapFuture, AsyncContext, Addr};
use actix::prelude::{Actor, Context};
use tokio::prelude::{
    future::{self, join_all, Future},
    stream::*,
};

use lazy_static::lazy_static;

use crate::log::prelude::*;
use tokio::runtime::Runtime;

pub type ShutdownMessageResult = Result<
    Box<dyn Future<Item = (), Error = Box<dyn std::error::Error + Send>> + std::marker::Send>,
    ()
>;

type ShutdownFutureType =
    dyn Future<
        Item = Vec<
            ()
            >,
        Error = std::boxed::Box<dyn std::error::Error + std::marker::Send>
    > + std::marker::Send
;

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

/// Send this when a signal is detected
struct ShutdownSignalDetected;

impl Message for ShutdownSignalDetected {
    type Result = ();
}

pub struct GracefulShutdown {
    /// [`Actor`]s to send message when graceful shutdown
    recipients: BTreeMap<u8, Vec<Recipient<ShutdownMessage>>>,

    /// Timeout after which all [`Actors`] will be forced shutdown
    shutdown_timeout: u64,
}

impl GracefulShutdown {
    fn new(shutdown_timeout: u64) -> Self {
        Self {
            recipients: BTreeMap::new(),
            shutdown_timeout,
        }
    }
}

impl Actor for GracefulShutdown {
    type Context = Context<Self>;
}

impl Handler<ShutdownSignalDetected> for GracefulShutdown {
    type Result = ();

    fn handle(&mut self, msg: ShutdownSignalDetected, ctx: &mut Context<Self>) {
//        match msg.0 {
//            SignalKind::Int => {
//                error!("SIGINT received, exiting");
//            }
//            SignalKind::Hup => {
//                error!("SIGHUP received, reloading");
//            }
//            SignalKind::Term => {
//                error!("SIGTERM received, stopping");
//            }
//            SignalKind::Quit => {
//                error!("SIGQUIT received, exiting");
//            }
//        };

        let mut tokio_runtime = Runtime::new().unwrap();

        if self.recipients.is_empty() {
            return;
        }

        let mut shutdown_future: Box<ShutdownFutureType> =
            Box::new(
                futures::future::ok(vec![])
            );

        for recipients_values in self.recipients.values() {

            let mut this_priority_futures_vec =
                Vec::with_capacity(self.recipients.len());

            for recipient in recipients_values {
                let (tx, rx) = channel();
                let tx2 = tx.clone();
                let send_future = recipient.send(ShutdownMessage {});

                tokio::spawn(
                    send_future
                        .map(move |res| {
                            tx.send(res);
                        })
                        .map_err(move |e| {
                            error!("Error sending shutdown message: {:?}", e);
                            tx2.send(Ok(Box::new(futures::future::ok(()))));
                        })
                );

                let recipient_shutdown_fut = rx.recv().unwrap().unwrap();
//            tokio_runtime.block_on(recipient_shutdown_fut);

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

        let shutdown_timeout_move = self.shutdown_timeout;
        ctx.run_later(
            Duration::from_millis(shutdown_timeout_move),
            move |_, _| {
                System::current().stop();
            },
        );

        if self.recipients.is_empty() {
            return;
        }

        ctx.spawn(
            shutdown_future
                .map_err(|e| {
                    error!(
                        "Error trying to shut down system gracefully: {:?}",
                        e
                    );
                })
                .then(|_| {
                    System::current().stop();
                    future::ok::<(), ()>(())
                })
                .into_actor(self),
        );
    }
}

impl Handler<ShutdownSubscribe> for GracefulShutdown {
    type Result = ();

    fn handle(&mut self, msg: ShutdownSubscribe, _: &mut Context<Self>) {
        // todo: may be a bug: may subscribe same address multiple times with
        // the same/different priorities

        let vec_with_current_priority = self.recipients.get_mut(&msg.priority);
        if let Some(vector) = vec_with_current_priority {
            vector.push(msg.who);
        } else {
            self.recipients.insert(msg.priority, Vec::new());
            // unwrap should not panic because we have inserted new empty vector
            // with the key we are trying to get in the line above /\
            let vector = self.recipients.get_mut(&msg.priority).unwrap();
            vector.push(msg.who);
        }
    }
}


pub fn create(shutdown_timeout: u64) -> Addr<GracefulShutdown> {
    let graceful_shutdown = GracefulShutdown::new(shutdown_timeout).start();
    let graceful_shutdown_recipient = graceful_shutdown.clone().recipient();
    #[cfg(not(unix))]
    {
        error!("Unable to use graceful_shutdown: only UNIX signals are supported");
        return graceful_shutdown;
    }
    #[cfg(unix)]
    {
        use tokio_signal::unix::{Signal, SIGHUP, SIGINT, SIGQUIT, SIGTERM};

         // SIGINT
        let sigint_stream = Signal::new(SIGINT).flatten_stream();
        let sigterm_stream = Signal::new(SIGTERM).flatten_stream();
        let sigquit_stream = Signal::new(SIGQUIT).flatten_stream();
        let sighup_stream = Signal::new(SIGHUP  ).flatten_stream();
        let signals_stream = sigint_stream
            .select(sigterm_stream)
            .select(sigquit_stream)
            .select(sighup_stream);

        let handler = move |signal| {
            graceful_shutdown_recipient.do_send(ShutdownSignalDetected {});
        };

        thread::spawn(move || {
            tokio::run(
                signals_stream.into_future()
                    .map(handler)
                    .map_err(|err| ())
            );
        });
    }

    graceful_shutdown
}
