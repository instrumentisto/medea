use crate::{
    api::control::RoomId,
    signalling::Room,
    turn::{cli::CoturnTelnetClient, TurnAuthService},
};
use actix::{
    Actor, ActorFuture, Addr, AsyncContext, Handler, Message, WrapFuture,
};
use medea_client_api_proto::PeerId;
use medea_coturn_telnet_client::sessions_parser::Session;
use std::{collections::HashMap, sync::Arc, time::Duration};

#[derive(Debug)]
pub struct StatsValidator {
    coturn_client: Arc<dyn TurnAuthService>,
}

impl StatsValidator {
    pub fn new(coturn_client: Arc<dyn TurnAuthService>) -> Self {
        Self { coturn_client }
    }
}

/// Ergonomic type alias for using [`ActorFuture`] for [`Room`].
pub type ActFuture<O> =
    Box<dyn ActorFuture<Actor = StatsValidator, Output = O>>;

impl Actor for StatsValidator {
    type Context = actix::Context<Self>;
}

#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct Validate {
    pub peer_id: PeerId,
    pub room_id: RoomId,
}

impl Handler<Validate> for StatsValidator {
    type Result = ActFuture<Result<(), ()>>;

    fn handle(
        &mut self,
        msg: Validate,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.room_id.clone();
        let peer_id = msg.peer_id;

        Box::new(tokio::time::delay_for(Duration::from_secs(3)).into_actor(self)
            .then(move |_, this, ctx| {
                let coturn_client = this.coturn_client.clone();
                    async move {
                        coturn_client.get_sessions(room_id, peer_id).await
                    }
                        .into_actor(this)
                        .map(|res, this, ctx| match res {
                            Ok(sessions) => {
                                let peer_traffic: u64 = sessions
                                    .into_iter()
                                    .map(|session| {
                                        session.traffic_usage.sent_packets
                                            + session.traffic_usage.received_packets
                                    })
                                    .sum();
                                if peer_traffic < 50 {
                                    Err(())
                                } else {
                                    Ok(())
                                }
                            }
                            Err(_) => {
                                Err(())
                            }
                        })
            }))
    }
}
