//! WebSocket session.

use std::time::{Duration, Instant};

use actix::{
    fut::wrap_future, Actor, ActorContext, Addr, AsyncContext, Handler,
    Message, StreamHandler,
};
use actix_web::ws::{self, CloseReason};
use futures::future::Future;
use serde::{Deserialize, Serialize};

use crate::{
    api::client::rpc_connection::{
        Closed, ClosedReason, Established, RpcConnection,
    },
    api::client::{Command, Event, Room},
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

    /// Timeout of receiving any messages from client.
    idle_timeout: Duration,

    /// Timestamp for watchdog which checks whether WebSocket client became
    /// idle (no messages received during [`idle_timeout`]).
    ///
    /// This one should be renewed on any received WebSocket message
    /// from client.
    last_activity: Instant,

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
            idle_timeout,
            last_activity: Instant::now(),
            closed_by_server: false,
        }
    }

    /// Start idle watchdog.
    fn start_watchdog(&mut self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(Duration::new(1, 0), |sess, ctx| {
            if Instant::now().duration_since(sess.last_activity)
                > sess.idle_timeout
            {
                info!("WsSession of member {} is idle", sess.member_id);
                if let Err(err) = sess.room.try_send(Closed {
                    member_id: sess.member_id,
                    reason: ClosedReason::Idle,
                }) {
                    error!(
                        "WsSession of member {} failed to remove from Room, \
                         because: {:?}",
                        sess.member_id, err,
                    )
                }

                ctx.notify(Close {
                    reason: Some(ws::CloseCode::Normal.into()),
                });
            }
        });
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

        self.start_watchdog(ctx);

        let member_id = self.member_id;
        let slf_addr = ctx.address();
        ctx.wait(wrap_future(
            self.room
                .send(Established {
                    member_id: self.member_id,
                    connection: Box::new(ctx.address()),
                })
                .map(|_| ())
                .map_err(move |err| {
                    error!(
                        "WsSession of member {} failed to join Room, because: \
                         {:?}",
                        member_id, err,
                    );
                    slf_addr.do_send(Close {
                        reason: Some(ws::CloseCode::Normal.into()),
                    });
                }),
        ));
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("Stopped WsSession for member {}", self.member_id);
    }
}

impl RpcConnection for Addr<WsSession> {
    /// Closes [`WsSession`] by sending itself "normal closure" close message.
    ///
    /// Never returns error.
    fn close(&mut self) -> Box<dyn Future<Item = (), Error = ()>> {
        let fut = self
            .send(Close {
                reason: Some(ws::CloseCode::Normal.into()),
            })
            .or_else(|_| Ok(()));
        Box::new(fut)
    }

    /// Sends [`Event`] to Web Client.
    fn send_event(
        &self,
        event: Event,
    ) -> Box<dyn Future<Item = (), Error = ()>> {
        let fut = self
            .send(event)
            .map_err(|err| error!("Failed send event {:?} ", err));
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

impl Handler<Event> for WsSession {
    type Result = ();

    /// Sends [`Event`] to Web Client.
    fn handle(&mut self, event: Event, ctx: &mut Self::Context) {
        debug!("Event {:?} for member {}", event, self.member_id);
        ctx.text(serde_json::to_string(&event).unwrap())
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
                self.last_activity = Instant::now();
                if let Ok(ping) = serde_json::from_str::<Heartbeat>(&text) {
                    ctx.notify(ping);
                }
                if let Ok(command) = serde_json::from_str::<Command>(&text) {
                    if let Err(err) = self.room.try_send(command) {
                        error!(
                            "Cannot send Command to Room {}, because {}",
                            self.member_id, err
                        )
                    }
                }
            }
            ws::Message::Close(reason) => {
                if !self.closed_by_server {
                    debug!(
                        "Send close frame with reason {:?} for member {}",
                        reason, self.member_id
                    );
                    if let Err(err) = self.room.try_send(Closed {
                        member_id: self.member_id,
                        reason: ClosedReason::Disconnected,
                    }) {
                        error!(
                            "WsSession of member {} failed to remove from \
                             Room, because: {:?}",
                            self.member_id, err,
                        )
                    };
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
