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
    api::client::{
        Command, Event, Room, RpcConnection, RpcConnectionClosed,
        RpcConnectionClosedReason, RpcConnectionEstablished,
    },
    api::control::member::Id as MemberId,
    log::prelude::*,
};

// TODO: via conf
/// Timeout of receiving any WebSocket messages from client.
pub const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Long-running WebSocket connection of Client API.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct WsConnection {
    /// ID of [`Member`] that WebSocket connection is associated with.
    member_id: MemberId,
    /// [`Room`] that [`Member`] is associated with.
    room: Addr<Room>,

    /// Timestamp for watchdog which checks whether WebSocket client became
    /// idle (no messages received during [`CLIENT_IDLE_TIMEOUT`]).
    ///
    /// This one should be renewed on any received WebSocket message
    /// from client.
    last_activity: Instant,

    /// Indicates whether WebSocket connection is closed by server ot by
    /// client.
    closed_by_server: bool,
}

impl WsConnection {
    /// Creates new [`WsSession`] for specified [`Member`].
    pub fn new(member_id: MemberId, room: Addr<Room>) -> Self {
        Self {
            member_id,
            room,
            last_activity: Instant::now(),
            closed_by_server: false,
        }
    }

    /// Start idle watchdog.
    fn start_watchdog(&mut self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(Duration::new(1, 0), |conn, ctx| {
            if Instant::now().duration_since(conn.last_activity)
                > CLIENT_IDLE_TIMEOUT
            {
                info!("WsConnection with member {} is idle", conn.member_id);

                let member_id = conn.member_id;
                ctx.wait(wrap_future(
                    conn.room
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
            }
        });
    }
}

/// [`Actor`] implementation that provides an ergonomic way to deal with
/// WebSocket connection lifecycle for [`WsSession`].
impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    /// Starts [`Heartbeat`] mechanism and sends [`RpcConnectionEstablished`]
    /// signal to the [`Room`].
    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Started WsSession for member {}", self.member_id);

        self.start_watchdog(ctx);

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
                        "WsConnection of member {} failed to join Room, \
                         because: {:?}",
                        member_id, err,
                    )
                }),
        ));
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("Stopped WsSession for member {}", self.member_id);
    }
}

impl RpcConnection for Addr<WsConnection> {
    /// Closes [`WsConnection`] by sending itself "normal closure" close
    /// message.
    fn close(&self) -> Box<dyn Future<Item = (), Error = ()>> {
        let fut = self
            .send(Close {
                reason: Some(ws::CloseCode::Normal.into()),
            })
            .map_err(|_| ());
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

impl Handler<Close> for WsConnection {
    type Result = ();

    /// Closes WebSocket connection and stops [`Actor`] of [`WsSession`].
    fn handle(&mut self, close: Close, ctx: &mut Self::Context) {
        debug!("Closing WsSession for member {}", self.member_id);
        self.closed_by_server = true;
        ctx.close(close.reason);
        ctx.stop();
    }
}

impl Handler<Event> for WsConnection {
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

impl Handler<Heartbeat> for WsConnection {
    type Result = ();

    /// Answers with `Heartbeat::Pong` message to WebSocket client in response
    /// to the received `Heartbeat::Ping` message.
    fn handle(&mut self, msg: Heartbeat, ctx: &mut Self::Context) {
        if let Heartbeat::Ping(n) = msg {
            ctx.text(serde_json::to_string(&Heartbeat::Pong(n)).unwrap())
        }
    }
}

impl StreamHandler<ws::Message, ws::ProtocolError> for WsConnection {
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
                    let member_id = self.member_id;
                    ctx.wait(wrap_future(
                        self.room
                            .send(command)
                            .map(move |res| {
                                match res {
                                    Ok(_) => (),
                                    Err(_) => error!(
                                        "Command {} from member {} handle \
                                         failed",
                                        text, member_id,
                                    ),
                                };
                            })
                            .map_err(move |err| {
                                error!(
                                    "Cannot send Command from member {}, \
                                     because {}",
                                    member_id, err
                                )
                            }),
                    ));
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
                                    "Cannot send Close from member {}, \
                                     because {}",
                                    member_id, err
                                )
                            }),
                    ));
                    ctx.close(reason);
                    // ctx.stop();
                }
            }
            _ => error!(
                "Unsupported client message from member {}",
                self.member_id
            ),
        }
    }
}
