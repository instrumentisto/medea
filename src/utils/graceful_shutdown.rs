//! A class to handle shutdown signals and to shut down system
//! Actix system has to be running for it to work.

use std::{collections::BTreeMap, mem, sync::Mutex, thread, time::Duration};

use actix::{self, MailboxError, Message, Recipient, System};
use tokio::prelude::{
    future::{join_all, Future},
    stream::*,
};

use lazy_static;

use crate::log::prelude::*;

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
    static ref STATIC_RECIPIENTS: Mutex<BTreeMap<u8, Vec<Recipient<ShutdownMessage>>>> =
        Mutex::new(BTreeMap::new());
    static ref STATIC_TIMEOUT: Mutex<u64> = Mutex::new(100_u64);
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

/// Subscribe to exit events, with priority
pub fn subscribe(who: Recipient<ShutdownMessage>, priority: u8) {
    // todo: may be a bug: may subscribe same address multiple times with
    // the same/different priorities

    let mut recipients = STATIC_RECIPIENTS.lock().unwrap();

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

fn handle_shutdown(msg: SignalKind) {
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

    let recipients = STATIC_RECIPIENTS.lock().unwrap();

    if recipients.is_empty() {
        return;
    }

    let mut shutdown_future: ShutdownFutureType =
        Box::new(futures::future::ok::<
            Vec<Result<(), Box<(dyn std::error::Error + Send + 'static)>>>,
            MailboxError,
        >(vec![Ok(())]));

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
        shutdown_future = Box::new(futures::future::ok::<
            Vec<Result<(), Box<(dyn std::error::Error + Send + 'static)>>>,
            MailboxError,
        >(vec![Ok(())]));
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
    let mut global_shutdown_timeout = STATIC_TIMEOUT.lock().unwrap();
    *global_shutdown_timeout = shutdown_timeout;

    #[cfg(unix)]
    {
        use tokio_signal::unix::{Signal, SIGHUP, SIGINT, SIGQUIT, SIGTERM};

        {
            // SIGINT
            let sigint_stream = Signal::new(SIGINT).flatten_stream();
            let actix_system_for_sigint = actix_system.clone();
            let sigint_handler = sigint_stream.for_each(move |_| {

                let actix_system_for_sigint_move = actix_system_for_sigint.clone();

                thread::spawn(move || {
                    let shutdown_timeout = STATIC_TIMEOUT.lock().unwrap();
                    thread::sleep(Duration::from_millis(*shutdown_timeout));
                    actix_system_for_sigint_move.stop();
                });

                self::handle_shutdown(SignalKind::Int);
                actix_system_for_sigint.stop();
                Ok(())
            });
            thread::spawn(move || {
                tokio::runtime::current_thread::block_on_all(sigint_handler)
                    .ok()
                    .unwrap();
            });
        }
        {
            // SIGTERM
            let sigint_stream = Signal::new(SIGTERM).flatten_stream();
            let actix_system_for_sigint = actix_system.clone();
            let sigint_handler = sigint_stream.for_each(move |_| {

                let actix_system_for_sigint_move = actix_system_for_sigint.clone();

                thread::spawn(move || {
                    let shutdown_timeout = STATIC_TIMEOUT.lock().unwrap();
                    thread::sleep(Duration::from_millis(*shutdown_timeout));
                    actix_system_for_sigint_move.stop();
                });

                self::handle_shutdown(SignalKind::Term);
                actix_system_for_sigint.stop();
                Ok(())
            });
            thread::spawn(move || {
                tokio::runtime::current_thread::block_on_all(sigint_handler)
                    .ok()
                    .unwrap();
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
}
