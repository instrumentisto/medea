//! WebSocket session.

use std::time::Duration;

use actix::prelude::*;
use actix_web::ws::{self, CloseReason};
use serde::{Deserialize, Serialize};

use crate::{
    api::client::room::{JoinMember, LeaveMember, Room},
    api::control::member::Id as MemberID,
    log::prelude::*,
};

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
}

impl WsSession {
    /// Creates new WebSocket session for specified [`Member`].
    pub fn new(member_id: MemberID, room: Addr<Room>) -> Self {
        Self {
            member_id,
            room,
            idle_handler: None,
        }
    }

    /// Resets idle handler watchdog.
    fn reset_idle_timeout(&mut self, ctx: &mut <Self as Actor>::Context) {
        if let Some(handler) = self.idle_handler {
            ctx.cancel_future(handler);
        }
        self.idle_handler =
            Some(ctx.run_later(CLIENT_IDLE_TIMEOUT, |session, _ctx| {
                debug!("Client timeout");
                session.room.do_send(LeaveMember(
                    session.member_id,
                    Some(ws::CloseCode::Away.into()),
                ));
            }));
    }
}

/// [`Actor`] implementation that provides an ergonomic way to deal with
/// WebSocket connection lifecycle for [`WsSession`].
impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    /// Starts [`Heartbeat`] mechanism and sends message to [`Room`].
    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Session of member {} started", self.member_id);
        self.reset_idle_timeout(ctx);
        self.room.do_send(JoinMember(self.member_id, ctx.address()));
    }
}

/// Message sending from [`Room`] for closing [`WsSession`].
#[derive(Message)]
pub struct Close(pub Option<CloseReason>);

impl Handler<Close> for WsSession {
    type Result = ();

    /// Closes WebSocket connection and stops [`Actor`] of [`WsSession`].
    fn handle(&mut self, close: Close, ctx: &mut Self::Context) {
        ctx.close(close.0);
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
        debug!("Received WS message: {:?}", msg);
        match msg {
            ws::Message::Text(text) => {
                if let Ok(msg) = serde_json::from_str::<Heartbeat>(&text) {
                    ctx.notify(msg);
                }
                self.reset_idle_timeout(ctx);
            }
            ws::Message::Close(reason) => {
                self.room.do_send(LeaveMember(self.member_id, reason))
            }
            _ => error!("Unsupported message"),
        }
    }
}
