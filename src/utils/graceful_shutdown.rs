//! A class to handle shutdown signals and to shut down system
//! Actix system has to be running for it to work.

use std::{collections::BTreeMap, mem, sync::Mutex, thread, time::Duration,
          sync::mpsc::channel};

use actix::{self, MailboxError, Message, Recipient, System};
use tokio::prelude::{
    future::{join_all, Future},
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
    let mut tokio_runtime = Runtime::new().unwrap();

    if recipients.is_empty() {
        return;
    }

    let mut shutdown_future: Box<ShutdownFutureType> =
            Box::new(
                futures::future::ok(vec![])
            );

    for recipients_values in recipients.values() {

        let mut this_priority_futures_vec =
            Vec::with_capacity(recipients.len());

        for recipient in recipients_values {
            let (tx, rx) = channel();
            let tx2 = tx.clone();
            let send_future = recipient.send(ShutdownMessage {});

            tokio_runtime.spawn(
                    send_future
//                        futures::future::err::
//                            <Box<dyn Future<Item = (), Error = Box<dyn std::error::Error + Send>> + std::marker::Send>, ()>
//                            (())
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

    tokio_runtime.block_on(
        shutdown_future
            .map(|_| ())
            .map_err(|e| {
                error!("Error trying to shut down system gracefully: {:?}", e);
            })
        );

    tokio_runtime.shutdown_on_idle()
        .wait().unwrap();
}

pub fn create(shutdown_timeout: u64, actix_system: System) {
    let mut global_shutdown_timeout = STATIC_STRUCT.timeout.lock().unwrap();
    *global_shutdown_timeout = shutdown_timeout;

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

        let actix_system_for_signal = actix_system.clone();
        let handler = move |signal| {
//            let actix_system_for_signal_move = actix_system_for_signal.clone();
//            thread::spawn(move || {
//                let shutdown_timeout = STATIC_STRUCT.timeout.lock().unwrap();
//                thread::sleep(Duration::from_millis(*shutdown_timeout));
//                actix_system_for_signal_move.stop();
//            });

            thread::spawn(move || {
                self::handle_shutdown();
                actix_system_for_signal.stop();
            });
        };

        thread::spawn(move || {
            tokio::runtime::current_thread::run(
                signals_stream.into_future()
                .map(handler)
                    .map_err(|err| ()));
        });
    }

}
