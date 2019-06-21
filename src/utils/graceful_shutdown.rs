//! A class to handle shutdown signals and to shut down system

use actix::{actors::signal::{self, ProcessSignals, Subscribe},
            fut::wrap_future, Actor, ActorFuture, Addr,
            AsyncContext, Context, Handler, Message, System, Recipient,
            Arbiter};
use std::thread::{
  self, sleep
};
use std::sync::mpsc;
use std::{time};

use crate::log::prelude::*;
use tokio::prelude::future::Future;
use actix::actors::signal::{
    Signal, SignalType
};
use std::time::{Duration, Instant};
use tokio::timer::Delay;
use std::sync::mpsc::{Sender, Receiver};


macro_rules! important_quit_signals {
    ($msg: expr, $action: expr) => {
        match $msg {
            SignalType::Int => {
                error!("SIGINT received, exiting");
                $action();
            }
            SignalType::Hup => {
                error!("SIGHUP received, reloading");
                $action();
            }
            SignalType::Term => {
                error!("SIGTERM received, stopping");
                $action();
            }
            SignalType::Quit => {
                error!("SIGQUIT received, exiting");
                $action();
            }
            _ => (),
        }
    };
}

pub struct GracefulShutdownResult;

impl Message for GracefulShutdownResult {
    type Result = Result<(), Box<dyn std::error::Error + Send>>;
}

pub struct GracefulShutdown {
    /// [`Actor`]s to send message when graceful shutdown
    recipients: Vec<Recipient<GracefulShutdownResult>>,

    /// Timeout after which all [`Actors`] will be forced shutdown
    shutdown_timeout: u64,

    /// Actix addr of [`ProcessSignals`]
    process_signals: Addr<ProcessSignals>,
}

impl GracefulShutdown {
    pub fn new(shutdown_timeout: u64, process_signals: Addr<ProcessSignals>) -> Self {
        Self {
            recipients: Vec::new(),
            shutdown_timeout,
            process_signals,
        }
    }

    pub fn subscribe(&mut self, who: Recipient<GracefulShutdownResult>) {
        self.recipients.push(who);
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

        let (sender, receiver) = mpsc::channel();

        sender.send(());

        important_quit_signals!(msg.0, {
            let mut send_futures = Vec::with_capacity(self.recipients.len());
            for recipient in &self.recipients {
                send_futures.push(recipient.send(GracefulShutdownResult{}));
            }

            for send_future in send_futures {
                let future_sender1 = sender.clone();
                let future_sender2 = sender.clone();
                Arbiter::spawn(
                    send_future
                            .map(move |res| {
                                future_sender1.send(());
                            })
                            .map_err(move |err| {
                                error!("Error shutting down: {:?}", err);
                                future_sender2.send(());
                            })
                );
            }
        });


        let receiver_length = self.recipients.len() as u64;

        let wait_for_send_futures_to_execute =
            futures::future::poll_fn(move ||
                tokio_threadpool::blocking(|| {
                    for _ in 0..receiver_length {
                        match receiver.recv() {
                            Ok(_) => {()},
                            Err(_) => { return (); },
                        };
                    }
                })
            );

        let timeout_future =
            Delay::new(Instant::now() + Duration::from_millis(self.shutdown_timeout));

        let finish_or_timeout =
            timeout_future.select2(
                wait_for_send_futures_to_execute
            );

        // !!!! this code PANICS 7 times on thread and 1 time on arbiter

        Arbiter::spawn(
            finish_or_timeout.map(|_| {
                System::current().stop();
            })
                .map_err(|_| {
                    System::current().stop();
                })
        );

//        ctx.run_later(time::Duration::from_millis(300), |_, _| {
//            System::current().stop();
//        });
//
//        tokio::spawn({
//            futures::future::empty()
//                .map(|_: ()| {
//                    thread::sleep(time::Duration::from_millis(self.shutdown_timeout));
//                    System::current().stop();
//                })
//                .map_err(|_: ()| ())
//        });
    }
}