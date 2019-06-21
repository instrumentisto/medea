//! A class to handle shutdown signals and to shut down system

use actix::{actors::signal::{self, ProcessSignals, Subscribe}, fut::wrap_future, Actor, ActorFuture, Addr, AsyncContext, Context, Handler, Message, System, Recipient};
use std::thread::{
  self, sleep
};
use std::time;

use crate::log::prelude::*;
use tokio::prelude::future::Future;
use actix::actors::signal::{
    Signal, SignalType
};


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

#[derive(Clone)]
pub struct GracefulShutdown {
    /// [`Actor`]s to send Signal to gracefully shutdown
    recipients: Vec<Recipient<Signal>>,

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

    pub fn subscribe(&mut self, who: Recipient<Signal>) {
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

        important_quit_signals!(msg.0, {
            for recipient in &self.recipients {
                recipient.do_send(Signal(msg.0.clone()));
            }
        });

        ctx.run_later(time::Duration::from_millis(self.shutdown_timeout), |_, _| {
            System::current().stop();
        });
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