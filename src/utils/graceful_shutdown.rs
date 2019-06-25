//! A class to handle shutdown signals and to shut down system

use std::{
    collections::BTreeMap,
    time::Duration
};

use actix::{
    Actor,
    actors::signal::{self, ProcessSignals, Subscribe}, actors::signal::{Signal, SignalType}, Addr, AsyncContext, Context, Handler, Message, prelude::fut::WrapFuture,
    Recipient,
    System,
};
use tokio::prelude::{
    future::{Future, join_all},
};

use crate::log::prelude::*;
use crate::utils::then_all::then_all;

#[derive(Debug)]
pub struct ShutdownResult;

impl Message for ShutdownResult {
    type Result = Result<(), Box<dyn std::error::Error + Send>>;
}

pub struct GracefulShutdown {
    /// [`Actor`]s to send message when graceful shutdown
    recipients: BTreeMap<u8, Vec<Recipient<ShutdownResult>>>,

    /// Timeout after which all [`Actors`] will be forced shutdown
    // shutdown_timeout: u64,

    /// Actix address of [`ProcessSignals`]
    process_signals: Addr<ProcessSignals>,
}

impl GracefulShutdown {
    pub fn new(process_signals: Addr<ProcessSignals>) -> Self {
        Self {
            recipients: BTreeMap::new(),
            // shutdown_timeout,
            process_signals,
        }
    }

    /// Subscribe to exit events, with priority
    // todo: may be a bug: no checking who subscribed, may subcsribe same adress
    // multiple times with the same/different priorities
    pub fn subscribe(
        &mut self,
        priority: u8,
        who: Recipient<ShutdownResult>,
    ) {
        let vec_with_current_priority = self.recipients.get_mut(&priority);
        if let Some(vector) = vec_with_current_priority {
            vector.push(who);
        } else {
            self.recipients.insert(priority, Vec::new());
            // unwrap should not panic because we have inserted new empty vector
            // in the line above /\
            let vector = self.recipients.get_mut(&priority).unwrap();
            vector.push(who);
        }
    }
}

impl Actor for GracefulShutdown {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.process_signals
            .do_send(Subscribe(ctx.address().recipient()));
    }
}

impl Handler<Signal> for GracefulShutdown {
    type Result = ();

    fn handle(&mut self, msg: Signal, ctx: &mut Context<Self>) {
        match msg.0 {
            SignalType::Int => {
                error!("SIGINT received, exiting");
            }
            SignalType::Hup => {
                error!("SIGHUP received, reloading");
            }
            SignalType::Term => {
                error!("SIGTERM received, stopping");
            }
            SignalType::Quit => {
                error!("SIGQUIT received, exiting");
            }
            _ => {
                return;
            }
        };

        if self.recipients.is_empty() {
            return;
        }

        let mut shutdown_futures_vec = Vec::new();

        for recipients in self.recipients.values() {
            let mut this_priority_futures_vec =
                Vec::with_capacity(recipients.len());

            for recipient in recipients {
                let send_fut = recipient.send(ShutdownResult {});
                this_priority_futures_vec.push(send_fut);
            }
            let this_priority_futures = join_all(this_priority_futures_vec);
            shutdown_futures_vec.push(this_priority_futures);
        }

        let shutdown_future = then_all(shutdown_futures_vec);

        ctx.wait(
            shutdown_future
                .map(|_| {
                    System::current().stop();
                })
                .map_err(|_| {
                    error!("Error trying to shut down system gracefully.");
                    System::current().stop();
                })
                .into_actor(self),
        );
    }
}

pub struct TimeoutShutdown {
    pub shutdown_timeout: u64,
    pub process_signals: Addr<ProcessSignals>,
}

impl Actor for TimeoutShutdown {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.process_signals
            .do_send(Subscribe(ctx.address().recipient()));
    }
}

impl Handler<Signal> for TimeoutShutdown {
    type Result = ();

    fn handle(&mut self, msg: Signal, ctx: &mut Context<Self>) {
        match msg.0 {
            SignalType::Int |
            SignalType::Hup |
            SignalType::Term |
            SignalType::Quit => {},
            _ => {
                return;
            }
        };
        info!(
            "System will be shut down in {:?} ms.",
            self.shutdown_timeout
        );

        ctx.run_later(
            Duration::from_millis(self.shutdown_timeout),
            move |_, _| {
                System::current().stop();
            },
        );
    }
}

pub fn new(
    shutdown_timeout: u64,
    process_signals: Addr<signal::ProcessSignals>,
) -> GracefulShutdown {
    let graceful_shutdown = GracefulShutdown::new(process_signals.clone());

    let timeout_shutdown = TimeoutShutdown {
        shutdown_timeout,
        process_signals,
    };
    timeout_shutdown.start();

    // todo impl messages for graceful shutdown and start it here
    graceful_shutdown
}
