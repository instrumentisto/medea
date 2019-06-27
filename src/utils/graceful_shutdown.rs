//! A class to handle shutdown signals and to shut down system

use std::{collections::BTreeMap, mem, time::Duration};

use actix::{
    self, prelude::fut::WrapFuture, Actor, Addr, Arbiter, AsyncContext,
    Context, Handler, MailboxError, Message, Recipient, System,
};

use tokio::prelude::future::{join_all, Future};

use crate::{log::prelude::*, utils::signal_handler::*};

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

#[derive(Debug)]
pub struct ShutdownResult;

impl Message for ShutdownResult {
    type Result = Result<(), Box<dyn std::error::Error + Send>>;
}

/// Subscribe to exit events, with priority
pub struct ShutdownSubscribe {
    pub priority: u8,
    pub who: Recipient<ShutdownResult>,
}

impl Message for ShutdownSubscribe {
    type Result = ();
}

pub struct GracefulShutdown {
    /// [`Actor`]s to send message when graceful shutdown
    recipients: BTreeMap<u8, Vec<Recipient<ShutdownResult>>>,

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

impl Handler<SignalMessage> for GracefulShutdown {
    type Result = ();

    fn handle(&mut self, msg: SignalMessage, ctx: &mut Context<Self>) {
        match msg.0 {
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

        let mut shutdown_future: ShutdownFutureType =
            Box::new(futures::future::ok::<
                Vec<Result<(), Box<(dyn std::error::Error + Send + 'static)>>>,
                MailboxError,
            >(vec![Ok(())]));

        for recipients in self.recipients.values() {
            let mut this_priority_futures_vec =
                Vec::with_capacity(recipients.len());

            for recipient in recipients {
                let send_fut = recipient.send(ShutdownResult {});
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

        ctx.run_later(
            Duration::from_millis(self.shutdown_timeout),
            move |_, _| {
                System::current().stop();
            },
        );

        if self.recipients.is_empty() {
            return;
        }

        ctx.spawn(
            shutdown_future
                .map(|_| {
                    System::current().stop();
                })
                .map_err(|e| {
                    error!(
                        "Error trying to shut down system gracefully: {:?}",
                        e
                    );
                    System::current().stop();
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
    let graceful_shutdown = GracefulShutdown::new(shutdown_timeout);
    let shutdown_arbiter = Arbiter::new();
    let graceful_shutdown_addr = Actor::start_in_arbiter(&shutdown_arbiter, |_| graceful_shutdown);
    SignalHandler::start(graceful_shutdown_addr.clone().recipient());
    graceful_shutdown_addr
}
