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
    },
    log::prelude::*,
    signalling::Room,
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

    /// Starts watchdog which will drop connection if now-last_activity >
    /// idle_timeout.
    fn start_watchdog(&mut self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(Duration::new(1, 0), |session, ctx| {
            if Instant::now().duration_since(session.last_activity)
                > session.idle_timeout
            {
                info!("WsSession of member {} is idle", session.member_id);
                if let Err(err) = session.room.try_send(RpcConnectionClosed {
                    member_id: session.member_id.clone(),
                    reason: ClosedReason::Lost,
                }) {
                    error!(
                        "WsSession of member {} failed to remove from Room, \
                         because: {:?}",
                        session.member_id, err,
                    )
                }
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
        debug!("Started WsSession for member {}", self.member_id);

        self.start_watchdog(ctx);

        ctx.wait(
            wrap_future(self.room.send(RpcConnectionEstablished {
                member_id: self.member_id.clone(),
                connection: Box::new(ctx.address()),
            }))
            .map(
                move |auth_result,
                      session: &mut Self,
                      ctx: &mut ws::WebsocketContext<Self>| {
                    if let Err(e) = auth_result {
                        error!(
                            "Room rejected Established for member {}, cause \
                             {:?}",
                            session.member_id, e
                        );
                        ctx.notify(Close::with_normal_code(
                            &CloseDescription::new(CloseReason::Rejected),
                        ));
                    }
                },
            )
            .map_err(
                move |send_err,
                      session: &mut Self,
                      ctx: &mut ws::WebsocketContext<Self>| {
                    error!(
                        "WsSession of member {} failed to join Room, because: \
                         {:?}",
                        session.member_id, send_err,
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
                        trace!("Received ping: {}", n);
                        // Answer with Heartbeat::Pong.
                        ctx.text(
                            serde_json::to_string(&ServerMsg::Pong(n)).unwrap(),
                        );
                    }
                    Ok(ClientMsg::Command(command)) => {
                        if let Err(err) =
                            self.room.try_send(CommandMessage::from(command))
                        {
                            error!(
                                "Cannot send Command to Room {}, because {}",
                                self.member_id, err
                            )
                        }
                    }
                    Err(err) => error!(
                        "Error [{:?}] parsing client message [{}]",
                        err, &text
                    ),
                }
            }
            ws::Message::Close(reason) => {
                if !self.closed_by_server {
                    if let Err(err) = self.room.try_send(RpcConnectionClosed {
                        member_id: self.member_id.clone(),
                        reason: ClosedReason::Closed,
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
