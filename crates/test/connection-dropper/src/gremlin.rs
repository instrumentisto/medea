use crate::{firewall::Firewall, prelude::*};
use actix::{
    Actor, AsyncContext, Context, Handler, Message, Running, SpawnHandle,
};
use rand::{rngs::ThreadRng, Rng};
use std::time::Duration;

pub struct Gremlin {
    dropper_handle: Option<SpawnHandle>,
    firewall: Firewall,
    rng: ThreadRng,
}

impl Gremlin {
    pub fn new(firewall: Firewall) -> Self {
        Self {
            dropper_handle: None,
            rng: rand::thread_rng(),
            firewall,
        }
    }

    pub fn step(&mut self, ctx: &mut Context<Gremlin>) {
        self.firewall
            .close_port(8090)
            .map_err(|e| warn!("Error while closing port: {:?}", e));
        self.dropper_handle = Some(ctx.run_later(
            Duration::from_secs(self.rng.gen_range(5, 15)),
            |gremlin, ctx| {
                gremlin
                    .firewall
                    .open_port(8090)
                    .map_err(|e| warn!("Error while opening port: {:?}", e));
                gremlin.dropper_handle = Some(ctx.run_later(
                    Duration::from_secs(gremlin.rng.gen_range(5, 15)),
                    |mut gremlin, mut ctx| {
                        gremlin.step(ctx);
                    },
                ));
            },
        ));
    }
}

impl Actor for Gremlin {
    type Context = Context<Self>;

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        self.firewall.open_port(8090);
        Running::Stop
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Start;

impl Handler<Start> for Gremlin {
    type Result = ();

    fn handle(&mut self, _: Start, ctx: &mut Self::Context) -> Self::Result {
        if let Some(handle) = self.dropper_handle.take() {
            ctx.cancel_future(handle);
            self.step(ctx);
        }
        self.step(ctx);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Stop;

impl Handler<Stop> for Gremlin {
    type Result = ();

    fn handle(&mut self, _: Stop, ctx: &mut Self::Context) -> Self::Result {
        if let Some(handle) = self.dropper_handle.take() {
            ctx.cancel_future(handle);
        }
        self.firewall.open_port(8090);
    }
}
