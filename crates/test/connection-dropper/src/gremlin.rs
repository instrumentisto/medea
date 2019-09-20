//! Implementation of service which can up/down connection at random time.

use std::time::Duration;

use actix::{
    Actor, AsyncContext, Context, Handler, Message, Running, SpawnHandle,
};
use rand::{rngs::ThreadRng, Rng};

use crate::{firewall::Firewall, prelude::*};

/// Service which can up/down connection at random time.
pub struct Gremlin {
    dropper_handle: Option<SpawnHandle>,
    firewall: Firewall,
    rng: ThreadRng,
}

impl Gremlin {
    /// Create new service which can up/down connection at random time.
    pub fn new(firewall: Firewall) -> Self {
        Self {
            dropper_handle: None,
            rng: rand::thread_rng(),
            firewall,
        }
    }

    /// Closes port for `Member`, up it after some random time, run
    /// `self.step()` after random time.
    ///
    /// This is recursive function. If you wish to stop it, you should call
    /// `ctx.cancel_future` for `self.dropper_handle` ([`Stop`] message will
    /// do it).
    pub fn step(&mut self, ctx: &mut <Self as Actor>::Context) {
        info!("Gremlin closes port.");
        self.firewall
            .close_port(8090)
            .map_err(|e| {
                self.firewall.open_port(8090).ok();
                e
            })
            .unwrap();

        self.dropper_handle = Some(ctx.run_later(
            Duration::from_secs(self.rng.gen_range(5, 15)),
            |gremlin, ctx| {
                info!("Gremlin opens port.");
                gremlin.firewall.open_port(8090).unwrap();
                gremlin.dropper_handle = Some(ctx.run_later(
                    Duration::from_secs(gremlin.rng.gen_range(5, 15)),
                    |gremlin, ctx| {
                        gremlin.step(ctx);
                    },
                ));
            },
        ));
    }
}

impl Actor for Gremlin {
    type Context = Context<Self>;

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        debug!("Shutdown gremlin.");
        self.firewall.open_port(8090).unwrap();
        Running::Stop
    }
}

/// Starts gremlin's up/down `Member` connection loop.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Start;

impl Handler<Start> for Gremlin {
    type Result = ();

    fn handle(&mut self, _: Start, ctx: &mut Self::Context) -> Self::Result {
        info!("Starting gremlin.");
        self.firewall.open_port(8090).unwrap();

        if let Some(handle) = self.dropper_handle.take() {
            debug!("Old dropper found. Cancelling old dropper's future.");
            ctx.cancel_future(handle);
        }
        self.step(ctx);
    }
}

/// Stops gremlin's up/down `Member` connection loop.
#[derive(Message)]
#[rtype(result = "()")]
pub struct Stop;

impl Handler<Stop> for Gremlin {
    type Result = ();

    fn handle(&mut self, _: Stop, ctx: &mut Self::Context) -> Self::Result {
        info!("Stopping gremlin.");
        if let Some(handle) = self.dropper_handle.take() {
            ctx.cancel_future(handle);
        }
        self.firewall.open_port(8090).unwrap();
    }
}
