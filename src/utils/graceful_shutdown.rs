//! A class to handle shutdown signals and to shut down system
//! Actix system has to be running for it to work.

// review
// remove all unwraps
// todo:
// i have done future based listener and shared resources between new instances. finish with main shutdown logic


use std::{collections::BTreeMap, mem, time::Duration, thread};
use std::sync::Mutex;

use actix::{
    self, prelude::fut::WrapFuture, Actor, Addr, Arbiter, AsyncContext,
    Context, Handler, MailboxError, Message, Recipient, System,
};
use futures::future;
use tokio::prelude::future::{join_all, Future};
use tokio::prelude::stream::*;


use lazy_static;

use crate::{log::prelude::*};
use std::sync::Arc;
use core::borrow::BorrowMut;
use std::rc::{Weak, Rc};

type ShutdownFutureType = Box<
    dyn Future<
        Item = std::vec::Vec<
            std::result::Result<
                (),
                std::boxed::Box<
                    (dyn std::error::Error + std::marker::Send + 'static),
                >,
            >,
        >,
        Error = MailboxError,
    >,
>;

lazy_static! {
    static ref StaticRecipients: Mutex<BTreeMap<u8, Vec<Recipient<ShutdownMessage>>>> = Mutex::new(BTreeMap::new());
    static ref StaticTimeout: Mutex<u64> = Mutex::new(0u64);
}

/// Different kinds of process signals
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum SignalKind {
    /// SIGHUP
    Hup,
    /// SIGINT
    Int,
    /// SIGTERM
    Term,
    /// SIGQUIT
    Quit,
}

#[derive(Debug)]
pub struct ShutdownMessage;

impl Message for ShutdownMessage {
    type Result = Result<(), Box<dyn std::error::Error + Send>>;
}

pub struct GracefulShutdown {
//    /// [`Actor`]s to send message when graceful shutdown
//    recipients: BTreeMap<u8, Vec<Recipient<ShutdownMessage>>>,
//
//    /// Timeout after which all [`Actors`] will be forced shutdown
//    shutdown_timeout: u64,
}

impl GracefulShutdown {
    fn new(mut shutdown_timeout: Option<u64>) -> Self {
        if let Some(shutdown_timeout_val) = shutdown_timeout {
            let mut timeout = StaticTimeout.lock().unwrap();
            *timeout = shutdown_timeout_val;
        }
        Self {
//            recipients: BTreeMap::new(),
//            shutdown_timeout,
        }
    }

    /// Subscribe to exit events, with priority
    pub fn subscribe(&mut self, who: Recipient<ShutdownMessage>, priority: u8) {
        // todo: may be a bug: may subscribe same address multiple times with
        // the same/different priorities

        let mut recipients = StaticRecipients.lock().unwrap();

        let vec_with_current_priority = recipients.get_mut(&priority);
        if let Some(vector) = vec_with_current_priority {
            vector.push(who);
        } else {
            recipients.insert(priority, Vec::new());
            // unwrap should not panic because we have inserted new empty vector
            // with the key we are trying to get in the line above /\
            let vector = recipients.get_mut(&priority).unwrap();
            vector.push(who);
        }
    }

    fn handle_shutdown(&mut self, msg: SignalKind) {
        match msg {
            SignalKind::Int => {
                error!("SIGINT received, exiting");
            }
            SignalKind::Hup => {
                error!("SIGHUP received, reloading");
            }
            SignalKind::Term => {
                error!("SIGTERM received, stopping");
            }
            SignalKind::Quit => {
                error!("SIGQUIT received, exiting");
            }
        };
//        let mut shutdown_future: ShutdownFutureType =
//            Box::new(futures::future::ok::<
//                Vec<Result<(), Box<(dyn std::error::Error + Send + 'static)>>>,
//                MailboxError,
//            >(vec![Ok(())]));
//
//        for recipients in self.recipients.values() {
//            println!("SHUTDOWN CAT: {} recipients", self.recipients.len());
//
//            let mut this_priority_futures_vec =
//                Vec::with_capacity(recipients.len());
//
//            for recipient in recipients {
//                let send_fut = recipient.send(ShutdownMessage {});
//                this_priority_futures_vec.push(send_fut);
//            }
//
//            let this_priority_futures = join_all(this_priority_futures_vec);
//            let new_shutdown_future =
//                Box::new(shutdown_future.then(|_| this_priority_futures));
//            // we need to rewrite shutdown_future, otherwise compiler thinks we
//            // moved value
//            shutdown_future = Box::new(futures::future::ok::<
//                Vec<Result<(), Box<(dyn std::error::Error + Send + 'static)>>>,
//                MailboxError,
//            >(vec![Ok(())]));
//            mem::replace(&mut shutdown_future, new_shutdown_future);
//        }
//
//    ctx.run_later(
//        Duration::from_millis(self.shutdown_timeout),
//        move |_, _| {
//            System::current().stop();
//        },
//    );
//
//        if self.recipients.is_empty() {
//            return;
//        }
//
//    ctx.spawn(
//        shutdown_future
//            .map_err(|e| {
//                error!(
//                    "Error trying to shut down system gracefully: {:?}",
//                    e
//                );
//            })
//            .then(|_| {
//                System::current().stop();
//                future::ok::<(), ()>(())
//            })
//            .into_actor(self),
//    );
    }
}

pub fn create(shutdown_timeout: u64) -> GracefulShutdown {
    let mut graceful_shutdown = GracefulShutdown::new(Some(shutdown_timeout));

//    let shutdown_arbiter = Arbiter::new();
//    let graceful_shutdown_addr =
//        Actor::start_in_arbiter(&shutdown_arbiter, |_| graceful_shutdown);
//    SignalHandler::start(graceful_shutdown_addr.clone().recipient());

    //TODO: Delete cfg(not(unix))
    #[cfg(not(unix))]
    {
        let ctrl_c = tokio_signal::ctrl_c().flatten_stream();

        let prog = ctrl_c.for_each(|_| {
            let mut graceful_shutdown = GracefulShutdown::new(None);
            graceful_shutdown.handle_shutdown(SignalKind::Int);
            Ok(())
        });

        thread::spawn(move || {
            tokio::runtime::current_thread::block_on_all(prog).unwrap();
        });
    }

//    #[cfg(unix)]
//    {
//        use tokio_signal::unix;
//
//        let stream = Signal::new(SIGHUP).flatten_stream();

//        let mut sigs: Vec<
//            Box<Future<Item = SigStream, Error = io::Error>>,
//        > = Vec::new();
//        sigs.push(Box::new(
//            tokio_signal::unix::Signal::new(tokio_signal::unix::SIGINT)
//                .map(|stream| {
//                    let s: SigStream =
//                        Box::new(stream.map(|_| SignalKind::Int));
//                    s
//                }),
//        ));
//        sigs.push(Box::new(
//            tokio_signal::unix::Signal::new(tokio_signal::unix::SIGHUP)
//                .map(|stream: unix::Signal| {
//                    let s: SigStream =
//                        Box::new(stream.map(|_| SignalKind::Hup));
//                    s
//                }),
//        ));
//        sigs.push(Box::new(
//            tokio_signal::unix::Signal::new(
//                tokio_signal::unix::SIGTERM,
//            )
//                .map(|stream| {
//                    let s: SigStream =
//                        Box::new(stream.map(|_| SignalKind::Term));
//                    s
//                }),
//        ));
//        sigs.push(Box::new(
//            tokio_signal::unix::Signal::new(
//                tokio_signal::unix::SIGQUIT,
//            )
//                .map(|stream| {
//                    let s: SigStream =
//                        Box::new(stream.map(|_| SignalKind::Quit));
//                    s
//                }),
//        ));
//
//        futures_unordered(sigs)
//            .collect()
//            .map_err(|_| ())
//            .and_then(move |streams| SignalHandler { srv, streams });
//    }

    graceful_shutdown
}
