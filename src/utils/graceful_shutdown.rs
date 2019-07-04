//! A class to handle shutdown signals and to shut down system
//! Actix system has to be running for it to work.

use std::{collections::BTreeMap, mem, sync::Mutex, thread, time::Duration};

use actix::{self, MailboxError, Message, Recipient, System};
use tokio::prelude::{
    future::{join_all, Future},
    stream::*,
};

use lazy_static::lazy_static;

use crate::log::prelude::*;

pub type ShutdownMessageResult = Result<
    Box<dyn Future<Item = (), Error = Box<dyn std::error::Error + Send>> + std::marker::Send>,
    ()>;

type ShutdownFutureType = Box<
    dyn Future<
        Item = std::vec::Vec<ShutdownMessageResult>,
        Error = MailboxError,
    >,
>;

//// TODO: why not use simple struct?
//lazy_static! {
//    static ref STATIC_RECIPIENTS: Mutex<BTreeMap<u8, Vec<Recipient<ShutdownMessage>>>> =
//        Mutex::new(BTreeMap::new());
//    static ref STATIC_TIMEOUT: Mutex<u64> = Mutex::new(100_u64);
//}

lazy_static!{
    static ref STATIC_STRUCT: StaticShutdown = StaticShutdown::new();
}

struct StaticShutdown {
    pub recipients: Mutex<BTreeMap<u8, Vec<Recipient<ShutdownMessage>>>>,
    pub timeout: Mutex<u64>,
}

impl StaticShutdown {
    fn new() -> Self {
        Self {
            recipients: Mutex::new(BTreeMap::new()),
            timeout: Mutex::new(100_u64),
        }
    }
}

#[derive(Debug)]
pub struct ShutdownMessage;

impl Message for ShutdownMessage {
    type Result = self::ShutdownMessageResult;
}

/// Subscribe to exit events, with priority
pub fn subscribe(who: Recipient<ShutdownMessage>, priority: u8) {
    // todo: may be a bug: may subscribe same address multiple times with
    // the same/different priorities

    let mut recipients = STATIC_STRUCT.recipients.lock().unwrap();

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

fn handle_shutdown() {
    error!("SIGINT, SIGHUP, SIGTERM or SIGQUIT received, exiting");

    let mut recipients = STATIC_STRUCT.recipients.lock().unwrap();

    if recipients.is_empty() {
        return;
    }

    let mut shutdown_future: ShutdownFutureType =
        Box::new(
            futures::future::ok::<
                        Vec<ShutdownMessageResult>,
                        MailboxError>
                (vec![Ok(Box::new(futures::future::ok(()) )) ] )
        );

    for recipients_values in recipients.values() {
        let mut this_priority_futures_vec =
            Vec::with_capacity(recipients.len());

        for recipient in recipients_values {
            let send_fut = recipient.send(ShutdownMessage {});
            this_priority_futures_vec.push(send_fut);
        }

        let this_priority_futures = join_all(this_priority_futures_vec);
        let new_shutdown_future =
            Box::new(shutdown_future.then(|_| this_priority_futures));
        // we need to rewrite shutdown_future, otherwise compiler thinks we
        // moved value
        shutdown_future = Box::new(
            futures::future::ok::<
                Vec<ShutdownMessageResult>,
                MailboxError,>
                (vec![Ok(Box::new(futures::future::ok(())))] ));

        mem::replace(&mut shutdown_future, new_shutdown_future);
    }

    let _ = shutdown_future
        .map(|_| ())
        .map_err(|e| {
            error!("Error trying to shut down system gracefully: {:?}", e);
        })
        .wait();
}

pub fn create(shutdown_timeout: u64, actix_system: System) {
    let mut global_shutdown_timeout = STATIC_STRUCT.timeout.lock().unwrap();

    *global_shutdown_timeout = shutdown_timeout;

    //TODO: this looks much less boilerplate:
    //    let int = Signal::new(SIGINT).flatten_stream();
    //    let term = Signal::new(SIGTERM).flatten_stream();
    //    let quit = Signal::new(SIGQUIT).flatten_stream();
    //
    //    let signal_stream = int.select(term).select(quit);
    //
    //    tokio::runtime::current_thread::run(signal_stream.into_future().and_then(move|(signal, _)|{
    //          my handler
    //    }).map_err(|err| ()));
    //

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


        let actix_system_for_sigint = actix_system.clone();
        let handler = move |signal| {

            let actix_system_for_sigint_move = actix_system_for_sigint.clone();

            thread::spawn(move || {
                let shutdown_timeout = STATIC_STRUCT.timeout.lock().unwrap();
                thread::sleep(Duration::from_millis(*shutdown_timeout));
//                    std::process::exit(0x0100);
                actix_system_for_sigint_move.stop();
            });

            self::handle_shutdown();
            actix_system_for_sigint.stop();
//                std::process::exit(0x0100);
        };

        thread::spawn(move || {
            tokio::runtime::current_thread::run(
                signals_stream.into_future()
                .map(handler)
                    .map_err(|err| ()));
        });
    }

//        {
//            // SIGHUP
//            let sighup_stream = Signal::new(SIGHUP).flatten_stream();
//            let actix_system_for_sighup_move = actix_system.clone();
//            let actix_system_for_sighup = actix_system.clone();
//            let sighup_handler = sighup_stream.for_each(move |_| {
//                thread::spawn( || {
//                    let shutdown_timeout = STATIC_TIMEOUT.lock().unwrap();
//                    thread::sleep(Duration::from_millis(*shutdown_timeout));
//                    actix_system_for_sighup_move.stop();
//                });
//                self::handle_shutdown(SignalKind::Hup);
//                actix_system_for_sighup.stop();
//                Ok(())
//            });
//            thread::spawn(move || {
//                tokio::runtime::current_thread::block_on_all(sighup_handler)
//                    .ok()
//                    .unwrap();
//            });
//        }
//        {
//            // SIGQUIT
//            let sigquit_stream = Signal::new(SIGQUIT).flatten_stream();
//            let actix_system_for_sigquit_move = actix_system.clone();
//            let actix_system_for_sigquit = actix_system.clone();
//            let sigquit_handler = sigquit_stream.for_each(move |_| {
//                thread::spawn(move || {
//                    let shutdown_timeout = STATIC_TIMEOUT.lock().unwrap();
//                    thread::sleep(Duration::from_millis(*shutdown_timeout));
//                    actix_system_for_sigquit_move.stop();
//                });
//                self::handle_shutdown(SignalKind::Quit);
//                actix_system_for_sigquit.stop();
//                Ok(())
//            });
//            thread::spawn(move || {
//                tokio::runtime::current_thread::block_on_all(sigquit_handler)
//                    .ok()
//                    .unwrap();
//            });
//        }
}
