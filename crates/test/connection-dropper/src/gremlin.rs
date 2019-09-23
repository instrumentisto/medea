//! Implementation of service which can up/down connection at random time.

use std::time::Duration;

use actix::{
    Actor, AsyncContext, Context, Handler, Message, Running, SpawnHandle,
};
use iptables::error::IPTResult;
use rand::{rngs::ThreadRng, Rng};

use crate::{firewall::Firewall, prelude::*};

/// Service which can up/down connection at random time.
pub struct Gremlin {
    /// [`SpawnHandle`] for `Member`'s connection up/down loop.
    dropper_handle: Option<SpawnHandle>,

    /// [`Firewall`] with which [`Gremlin`] will drop connection.
    firewall: Firewall,

    /// [`rand::ThreadRng`] for generating random down period in some range.
    rng: ThreadRng,

    /// Port which will be closed/opened.
    port_to_drop: u16,
}

impl Gremlin {
    /// Create new service which can up/down connection at random time.
    pub fn new(port_to_drop: u16, firewall: Firewall) -> Self {
        Self {
            dropper_handle: None,
            rng: rand::thread_rng(),
            firewall,
            port_to_drop,
        }
    }

    /// Closes port for `Member`, up it after some random time, run
    /// `self.step()` after random time.
    ///
    /// This is recursive function. If you wish to stop it, you should call
    /// `ctx.cancel_future` for `self.dropper_handle` ([`Stop`] message will
    /// do it).
    pub fn step(
        &mut self,
        ctx: &mut <Self as Actor>::Context,
    ) -> IPTResult<()> {
        info!("Gremlin closes port.");
        // TODO: maybe try to open port if err??
        self.close()?;

        self.dropper_handle = Some(ctx.run_later(
            Duration::from_secs(self.rng.gen_range(5, 15)),
            |gremlin, ctx| {
                info!("Gremlin opens port.");
                gremlin.open().unwrap();

                gremlin.dropper_handle = Some(ctx.run_later(
                    Duration::from_secs(gremlin.rng.gen_range(5, 15)),
                    |gremlin, ctx| {
                        gremlin.step(ctx).unwrap();
                    },
                ));
            },
        ));

        Ok(())
    }

    pub fn open(&self) -> IPTResult<bool> {
        self.firewall.open_port(self.port_to_drop)
    }

    pub fn close(&self) -> IPTResult<bool> {
        self.firewall.close_port(self.port_to_drop)
    }
}

impl Actor for Gremlin {
    type Context = Context<Self>;

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        debug!("Shutdown gremlin.");
        self.open().unwrap();

        Running::Stop
    }
}

/// Starts gremlin's up/down `Member` connection loop.
#[derive(Message)]
#[rtype(result = "IPTResult<()>")]
pub struct Start;

impl Handler<Start> for Gremlin {
    type Result = IPTResult<()>;

    fn handle(&mut self, _: Start, ctx: &mut Self::Context) -> Self::Result {
        info!("Starting gremlin.");
        self.open()?;

        if let Some(handle) = self.dropper_handle.take() {
            debug!("Old dropper found. Cancelling old dropper's future.");
            ctx.cancel_future(handle);
        }

        self.step(ctx)?;

        Ok(())
    }
}

/// Stops gremlin's up/down `Member` connection loop.
#[derive(Message)]
#[rtype(result = "IPTResult<()>")]
pub struct Stop;

impl Handler<Stop> for Gremlin {
    type Result = IPTResult<()>;

    fn handle(&mut self, _: Stop, ctx: &mut Self::Context) -> Self::Result {
        info!("Stopping gremlin's dropper task.");

        if let Some(handle) = self.dropper_handle.take() {
            ctx.cancel_future(handle);
        }

        self.open()?;

        Ok(())
    }
}
