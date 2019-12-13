//! WebSocket session.

use std::time::{Duration, Instant};

use actix::{
    fut::wrap_future, Actor, ActorContext, ActorFuture, Addr, AsyncContext,
    Handler, Message, StreamHandler,
};
use actix_web_actors::ws;
use futures::future::Future;
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason, ServerMsg,
};

use crate::{
    api::{
        client::rpc_connection::{
            ClosedReason, CommandMessage, EventMessage, RpcConnection,
            RpcConnectionClosed, RpcConnectionEstablished,
        },
        control::MemberId,
        RpcServer,
    },
    log::prelude::*,
};

/// Long-running WebSocket connection of Client API.
#[derive(Debug)]
pub struct WsSession {
    /// ID of [`Member`] that WebSocket connection is associated with.
    member_id: MemberId,

    /// [`Room`] that [`Member`] is associated with.
    room: Box<dyn RpcServer>,

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
        room: Box<dyn RpcServer>,
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

    /// Starts watchdog which will drop connection if `now`-`last_activity` >
    /// `idle_timeout`.
    fn start_watchdog(ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(Duration::new(1, 0), |session, ctx| {
            if Instant::now().duration_since(session.last_activity)
                > session.idle_timeout
            {
                info!("WsSession of member {} is idle", session.member_id);

                ctx.spawn(wrap_future(session.room.send_closed(
                    RpcConnectionClosed {
                        member_id: session.member_id.clone(),
                        reason: ClosedReason::Lost,
                    },
                )));

                ctx.notify(Close::with_normal_code(&CloseDescription::new(
                    CloseReason::Idle,
                )))
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
        debug!("Started WsSession for Member [id = {}]", self.member_id);

        Self::start_watchdog(ctx);

        ctx.wait(
            wrap_future(self.room.send_established(RpcConnectionEstablished {
                member_id: self.member_id.clone(),
                connection: Box::new(ctx.address()),
            }))
            .map_err(
                move |err,
                      session: &mut Self,
                      ctx: &mut ws::WebsocketContext<Self>| {
                    error!(
                        "WsSession of member {} failed to join Room, because: \
                         {:?}",
                        session.member_id, err,
                    );
                    ctx.notify(Close::with_normal_code(
                        &CloseDescription::new(CloseReason::InternalError),
                    ));
                },
            ),
        );
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("Stopped WsSession for member {}", self.member_id);
    }
}

impl RpcConnection for Addr<WsSession> {
    /// Closes [`WsSession`] by sending itself "normal closure" close message
    /// with [`CloseDescription`] as description of [Close] frame.
    ///
    /// Never returns error.
    ///
    /// [Close]: https://tools.ietf.org/html/rfc6455#section-5.5.1
    fn close(
        &mut self,
        close_description: CloseDescription,
    ) -> Box<dyn Future<Item = (), Error = ()>> {
        let fut = self
            .send(Close::with_normal_code(&close_description))
            .or_else(|_| Ok(()));

        Box::new(fut)
    }

    /// Sends [`Event`] to Web Client.
    ///
    /// [`Event`]: medea_client_api_proto::Event
    fn send_event(
        &self,
        msg: EventMessage,
    ) -> Box<dyn Future<Item = (), Error = ()>> {
        let fut = self
            .send(msg)
            .map_err(|err| warn!("Failed send event {:?} ", err));
        Box::new(fut)
    }
}

/// Message for closing [`WsSession`].
#[derive(Message)]
pub struct Close(ws::CloseReason);

impl Close {
    /// Creates [`Close`] message with [`ws::CloseCode::Normal`] and provided
    /// [`CloseDescription`] as serialized description.
    fn with_normal_code(description: &CloseDescription) -> Self {
        Self(ws::CloseReason {
            code: ws::CloseCode::Normal,
            description: Some(serde_json::to_string(&description).unwrap()),
        })
    }
}

impl Handler<Close> for WsSession {
    type Result = ();

    /// Closes WebSocket connection and stops [`Actor`] of [`WsSession`].
    fn handle(&mut self, close: Close, ctx: &mut Self::Context) {
        debug!("Closing WsSession for member {}", self.member_id);
        self.closed_by_server = true;
        ctx.close(Some(close.0));
        ctx.stop();
    }
}

impl Handler<EventMessage> for WsSession {
    type Result = ();

    /// Sends [`Event`] to Web Client.
    fn handle(&mut self, msg: EventMessage, ctx: &mut Self::Context) {
        let event =
            serde_json::to_string(&ServerMsg::Event(msg.into())).unwrap();
        debug!("Event {} for member {}", event, self.member_id);
        ctx.text(event);
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
                match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(ClientMsg::Ping(n)) => {
                        // Answer with Heartbeat::Pong.
                        ctx.text(
                            serde_json::to_string(&ServerMsg::Pong(n)).unwrap(),
                        );
                    }
                    Ok(ClientMsg::Command(command)) => {
                        ctx.spawn(wrap_future(
                            self.room
                                .send_command(CommandMessage::from(command)),
                        ));
                    }
                    Err(err) => error!(
                        "Error [{:?}] parsing client message [{}]",
                        err, &text
                    ),
                }
            }
            ws::Message::Close(reason) => {
                if !self.closed_by_server {
                    ctx.spawn(wrap_future(self.room.send_closed(
                        RpcConnectionClosed {
                            member_id: self.member_id.clone(),
                            reason: ClosedReason::Closed,
                        },
                    )));

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

#[cfg(test)]
mod test {

    use std::time::Duration;

//    use actix::Actor;
    use actix_web_actors::ws::WebsocketContext;

    use crate::api::{MockRpcServer, control::MemberId};

    use super::WsSession;

    #[test]
    fn close_if_rpc_established_failed() {
        let sys = actix::System::new("close_if_rpc_established_failed");

        let member_id = MemberId::from(String::from("test_member"));
        let rpc_server = MockRpcServer::new();
        let idle_timeout = Duration::from_secs(5);

        rpc_server.

        let ws_session = WsSession::new(member_id, Box::new(rpc_server), idle_timeout);
        let stream = futures::stream::empty();

        let asd = WebsocketContext::create_with_addr(ws_session, stream);

    }
}
