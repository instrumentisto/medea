//! A class to handle shutdown signals and to shut down system

use std::collections::BTreeMap;
use std::thread::{
    self, sleep
};
use std::sync::mpsc;
use std::{time};


use actix::{actors::signal::{self, ProcessSignals, Subscribe}, Actor, ActorFuture, Addr, AsyncContext, Context, Handler, Message, System, Recipient, Arbiter, MailboxError};

use crate::log::prelude::*;

use tokio::prelude::*;
use tokio::prelude::future::join_all;
use actix::actors::signal::{
    Signal, SignalType
};

use std::time::{Duration, Instant};
use tokio::timer::Delay;
use std::sync::mpsc::{Sender, Receiver};
use actix::prelude::fut::WrapFuture;
use core::borrow::BorrowMut;
use crate::utils::then_all::then_all;

#[derive(Debug)]
pub struct GracefulShutdownResult;

impl Message for GracefulShutdownResult {
    type Result = Result<(), Box<dyn std::error::Error + Send>>;
}

pub struct GracefulShutdown {
    /// [`Actor`]s to send message when graceful shutdown
    recipients: BTreeMap<u8, Vec<Recipient<GracefulShutdownResult>>>,

    /// Timeout after which all [`Actors`] will be forced shutdown
    shutdown_timeout: u64,

    /// Actix address of [`ProcessSignals`]
    process_signals: Addr<ProcessSignals>,
}

impl GracefulShutdown {
    pub fn new(shutdown_timeout: u64, process_signals: Addr<ProcessSignals>) -> Self {
        Self {
            recipients: BTreeMap::new(),
            shutdown_timeout,
            process_signals,
        }
    }

    /// Subscribe to exit events, with priority
    // todo: may be a bug: no checking who subscribed, may subcsribe same adress multiple times with the same/different priorities
    pub fn subscribe(&mut self, priority: u8, who: Recipient<GracefulShutdownResult>) {
        let vec_with_current_priority =
            self.recipients.get_mut(&priority);
        if let Some(vector) =
            vec_with_current_priority {
            vector.push(who);
        }
        else {
            self.recipients.insert(priority, Vec::new());
            // unwrap should not panic because we have inserted new empty vector in the line above /\
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
            _ => { return; },
        };


//        let mut shutdown_future: Box<future::Future<Item = (), Error = ()>> =
//            Box::new(
//                tokio::prelude::future::ok::<(), ()> (())
//            );

        let mut shutdown_futures_vec = Vec::new();

        for (priority, recipients) in &self.recipients {
            info!("PRINT FROM HANDLER: {:?}: \"{:?}\"", priority, recipients.len());

            let mut this_priority_futures_vec = Vec::with_capacity(recipients.len());

            for recipient in recipients {
                let send_fut = recipient.send(GracefulShutdownResult{});
                this_priority_futures_vec.push(send_fut);
//                shutdown_futures_vec.push(send_fut);
            }
            let this_priority_futures = join_all(this_priority_futures_vec);
            shutdown_futures_vec.push(this_priority_futures);
//            shutdown_future = Box::new(
//                shutdown_future
//                .then(|_| {
//                    this_priority_futures
//                        .map(|_| ())
//                        .map_err(|_| ())
//            }));
        }


        let shutdown_future = then_all(shutdown_futures_vec);

//        let timeout_future = Delay::new(Instant::now() + Duration::from_millis(self.shutdown_timeout));

//        let result = timeout_future.select2(shutdown_future);

        info!("PRINT FROM HANDLER: result ready");

        //todo handle panics and not freeze when that happends
        ctx.wait(
            shutdown_future
                .timeout(Duration::from_millis(self.shutdown_timeout))
                .map(|_| {
                    System::current().stop(); })
                .map_err(|_| {
                    error!("Error trying to shut down system gracefully.");
                    System::current().stop();
                })
                .into_actor(self)
        );
    }
}

