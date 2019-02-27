//! WebSocket session.

use std::time::Duration;

use actix::prelude::*;
use actix_web::ws::{self, CloseReason};
use serde::{Deserialize, Serialize};

use crate::{
    api::client::room::{
        Room, RpcConnection, RpcConnectionClosed, RpcConnectionClosedReason,
        RpcConnectionEstablished,
    },
    api::control::member::Id as MemberID,
    log::prelude::*,
};
use actix_web::ws::CloseCode;

// TODO: via conf
/// Timeout of receiving any WebSocket messages from client.
pub const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Long-running WebSocket connection of Client API.
#[derive(Debug)]
pub struct WsSession {
    /// ID of [`Member`] that WebSocket connection is associated with.
    member_id: MemberID,

    /// [`Room`] that [`Member`] is associated with.
    room: Addr<Room>,

    /// Handle for watchdog which checks whether WebSocket client became
    /// idle (no `ping` messages received during [`CLIENT_IDLE_TIMEOUT`]).
    ///
    /// This one should be renewed on any received WebSocket message
    /// from client.
    idle_handler: Option<SpawnHandle>,

    closed_by_server: bool,
}

impl WsSession {
    /// Creates new WebSocket session for specified [`Member`].
    pub fn new(member_id: MemberID, room: Addr<Room>) -> Self {
        Self {
            member_id,
            room,
            idle_handler: None,
            closed_by_server: false,
        }
    }

    /// Resets idle handler watchdog.
    fn reset_idle_timeout(&mut self, ctx: &mut <Self as Actor>::Context) {
        if let Some(handler) = self.idle_handler {
            ctx.cancel_future(handler);
        }

        let member_id = self.member_id;
        self.idle_handler =
            Some(ctx.run_later(CLIENT_IDLE_TIMEOUT, move |session, ctx| {
                info!("WsConnection with member {} is idle", member_id);
                ctx.notify(Close {
                    reason: Some(CloseCode::Normal.into()),
                });
                session.room.do_send(RpcConnectionClosed {
                    member_id: session.member_id,
                    reason: RpcConnectionClosedReason::Idle,
                });
            }));
    }
}

/// [`Actor`] implementation that provides an ergonomic way to deal with
/// WebSocket connection lifecycle for [`WsSession`].
impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    /// Starts [`Heartbeat`] mechanism and sends message to [`Room`].
    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Started WsSession for member {}", self.member_id);
        self.reset_idle_timeout(ctx);
        self.room.do_send(RpcConnectionEstablished {
            member_id: self.member_id,
            connection: Box::new(ctx.address()),
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("Stopped WsSession for member {}", self.member_id);
    }
}

impl RpcConnection for Addr<WsSession> {
    /// Close [`WsSession`] by send himself close message.
    fn close(&self) {
        debug!("Reconnect WsSession");
        self.do_send(Close {
            reason: Some(CloseCode::Normal.into()),
        });
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
#[derive(Debug, Message, Deserialize, Serialize)]
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
                    self.room.do_send(RpcConnectionClosed {
                        member_id: self.member_id,
                        reason: RpcConnectionClosedReason::Disconnect,
                    });
                    debug!("Send close frame with reason {:?}", reason);
                    ctx.close(reason);
                    ctx.stop();
                }
            }
            _ => error!("Unsupported message from member {}", self.member_id),
        }
    }
}
