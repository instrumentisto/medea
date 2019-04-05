//! WebSocket session.

use std::time::Duration;

use actix::{
    fut::wrap_future, Actor, ActorContext, Addr, AsyncContext, Handler,
    Message, SpawnHandle, StreamHandler,
};
use actix_web::ws::{self, CloseReason};
use futures::Future;
use serde::{Deserialize, Serialize};

use crate::{
    api::client::room::{
        Room, RpcConnection, RpcConnectionClosed, RpcConnectionClosedReason,
        RpcConnectionEstablished,
    },
    api::control::member::Id as MemberId,
    log::prelude::*,
};

/// Long-running WebSocket connection of Client API.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct WsSession {
    /// ID of [`Member`] that WebSocket connection is associated with.
    member_id: MemberId,
    /// [`Room`] that [`Member`] is associated with.
    room: Addr<Room>,

    /// Handle for watchdog which checks whether WebSocket client became
    /// idle (no `ping` messages received during [`idle_timeout`]).
    ///
    /// This one should be renewed on received `ping` message from client.
    idle_handler: Option<SpawnHandle>,
    /// Timeout of receiving `ping` messages from client.
    idle_timeout: Duration,

    /// Indicates whether WebSocket connection is closed by server ot by
    /// client.
    closed_by_server: bool,
}

impl WsSession {
    /// Creates new [`WsSession`] for specified [`Member`].
    pub fn new(
        member_id: MemberId,
        room: Addr<Room>,
        idle_timeout: Duration,
    ) -> Self {
        Self {
            member_id,
            room,
            idle_handler: None,
            idle_timeout,
            closed_by_server: false,
        }
    }

    /// Resets idle handler watchdog.
    fn reset_idle_timeout(&mut self, ctx: &mut <Self as Actor>::Context) {
        if let Some(handler) = self.idle_handler {
            ctx.cancel_future(handler);
        }

        self.idle_handler =
            Some(ctx.run_later(self.idle_timeout, |sess, ctx| {
                info!("WsConnection with member {} is idle", sess.member_id);

                let member_id = sess.member_id;
                ctx.wait(wrap_future(
                    sess.room
                        .send(RpcConnectionClosed {
                            member_id,
                            reason: RpcConnectionClosedReason::Idle,
                        })
                        .map_err(move |err| {
                            error!(
                                "WsSession of member {} failed to remove from \
                                 Room, because: {:?}",
                                member_id, err,
                            )
                        }),
                ));

                ctx.notify(Close {
                    reason: Some(ws::CloseCode::Normal.into()),
                });
            }));
    }
}

/// [`Actor`] implementation that provides an ergonomic way to deal with
/// WebSocket connection lifecycle for [`WsSession`].
impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    /// Starts [`Heartbeat`] mechanism and sends [`RpcConnectionEstablished`]
    /// signal to the [`Room`].
    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Started WsSession for member {}", self.member_id);

        self.reset_idle_timeout(ctx);

        let member_id = self.member_id;
        ctx.wait(wrap_future(
            self.room
                .send(RpcConnectionEstablished {
                    member_id: self.member_id,
                    connection: Box::new(ctx.address()),
                })
                .map(|_| ())
                .map_err(move |err| {
                    error!(
                        "WsSession of member {} failed to join Room, because: \
                         {:?}",
                        member_id, err,
                    )
                }),
        ));
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("Stopped WsSession for member {}", self.member_id);
    }
}

impl RpcConnection for Addr<WsSession> {
    /// Closes [`WsSession`] by sending itself "normal closure" close message.
    fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>> {
        let fut = self
            .send(Close {
                reason: Some(ws::CloseCode::Normal.into()),
            })
            .map_err(|_| ());
        Box::new(fut)
    }
}

/// Message for closing [`WsSession`].
#[derive(Message)]
pub struct Close {
    reason: Option<CloseReason>,
}

impl Handler<Close> for WsSession {
    type Result = ();

    /// Closes WebSocket connection and stops [`Actor`] of [`WsSession`].
    fn handle(&mut self, close: Close, ctx: &mut Self::Context) {
        debug!("Closing WsSession for member {}", self.member_id);
        self.closed_by_server = true;
        ctx.close(close.reason);
        ctx.stop();
    }
}

/// Message for keeping client WebSocket connection alive.
#[derive(Debug, Deserialize, Message, Serialize)]
pub enum Heartbeat {
    /// `ping` message that WebSocket client is expected to send to the server
    /// periodically.
    #[serde(rename = "ping")]
    Ping(usize),
    /// `pong` message that server answers with to WebSocket client in response
    /// to received `ping` message.
    #[serde(rename = "pong")]
    Pong(usize),
}

impl Handler<Heartbeat> for WsSession {
    type Result = ();

    /// Answers with `Heartbeat::Pong` message to WebSocket client in response
    /// to the received `Heartbeat::Ping` message.
    fn handle(&mut self, msg: Heartbeat, ctx: &mut Self::Context) {
        if let Heartbeat::Ping(n) = msg {
            trace!("Received ping: {}", n);
            ctx.text(serde_json::to_string(&Heartbeat::Pong(n)).unwrap())
        }
    }
}

impl StreamHandler<ws::Message, ws::ProtocolError> for WsSession {
    /// Handles arbitrary [`ws::Message`] received from WebSocket client.
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        debug!(
            "Received WS message: {:?} from member {}",
            msg, self.member_id
        );
        match msg {
            ws::Message::Text(text) => {
                self.reset_idle_timeout(ctx);
                if let Ok(msg) = serde_json::from_str::<Heartbeat>(&text) {
                    ctx.notify(msg);
                }
            }
            ws::Message::Close(reason) => {
                if !self.closed_by_server {
                    debug!(
                        "Send close frame with reason {:?} for member {}",
                        reason, self.member_id
                    );
                    let member_id = self.member_id;
                    ctx.wait(wrap_future(
                        self.room
                            .send(RpcConnectionClosed {
                                member_id: self.member_id,
                                reason: RpcConnectionClosedReason::Disconnected,
                            })
                            .map_err(move |err| {
                                error!(
                                    "WsSession of member {} failed to remove \
                                     from Room, because: {:?}",
                                    member_id, err,
                                )
                            }),
                    ));
                    ctx.close(reason);
                    ctx.stop();
                }
            }
            _ => error!(
                "Unsupported client message from member {}",
                self.member_id
            ),
        }
    }
}
