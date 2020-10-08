//! WebSocket session.

use std::{
    collections::HashMap,
    convert::TryInto as _,
    fmt::{Debug, Display, Error, Formatter},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use actix::{
    Actor, ActorContext, ActorFuture, Addr, Arbiter, AsyncContext,
    ContextFutureSpawner, Handler, MailboxError, Message, SpawnHandle,
    StreamHandler, WrapFuture,
};
use actix_http::ws::{CloseReason as WsCloseReason, Item};
use actix_web_actors::ws::{self, CloseCode};
use bytes::{Buf, BytesMut};
use futures::future::{FutureExt as _, LocalBoxFuture};
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason, Credentials, Event, MemberId,
    RoomId, RpcSettings, ServerMsg,
};

use crate::{
    api::{
        client::rpc_connection::{
            ClosedReason, EventMessage, RpcConnection, RpcConnectionSettings,
        },
        RpcServer,
    },
    log::prelude::*,
};

/// Repository of the all [`RpcServer`]s registered on this Media Server.
#[cfg_attr(test, mockall::automock)]
pub trait RpcServerRepository: Debug {
    /// Returns [`RpcServer`] with a provided [`RoomId`].
    ///
    /// Returns `None` if [`RpcServer`] with a provided [`RoomId`] doesn't
    /// exists.
    fn get(&self, room_id: &RoomId) -> Option<Box<dyn RpcServer>>;
}

#[cfg(test)]
impl_debug_by_struct_name!(MockRpcServerRepository);

/// Used to generate [`WsSession`] IDs.
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);
/// [`WsSession`] authentication timeout.
///
/// When this [`Duration`] will pass, [`WsSession`] will check that at least one
/// success authorization was happened.
#[cfg(not(test))]
static AUTH_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
static AUTH_TIMEOUT: Duration = Duration::from_secs(1);

/// [`WsSession`] closed reason.
#[derive(Clone, Copy, Debug)]
enum InnerCloseReason {
    /// [`WsSession`] was closed by [`RpcServer`] or was considered idle.
    ByServer,

    /// [`WsSession`] was closed by remote client.
    ByClient(ClosedReason),
}

/// Long-running WebSocket connection of Client API.
#[derive(Debug)]
pub struct WsSession {
    /// ID of [`WsSession`].
    id: u64,

    /// Repository of the all [`RpcServer`]s registered on this Media Server.
    rpc_server_repo: Box<dyn RpcServerRepository>,

    /// All sessions which this [`WsSession`] is serves.
    sessions: HashMap<RoomId, (MemberId, Box<dyn RpcServer>)>,

    /// Timeout of receiving any messages from client.
    idle_timeout: Duration,

    /// Timestamp for watchdog which checks whether WebSocket client became
    /// idle (no messages received during [`idle_timeout`]).
    ///
    /// This one should be renewed on any received WebSocket message
    /// from client.
    last_activity: Instant,

    /// Buffer where continuation WebSocket frames are accumulated.
    fragmentation_buffer: BytesMut,

    /// Last number of [`ServerMsg::Ping`].
    last_ping_num: u32,

    /// Interval to send [`ServerMsg::Ping`]s to a client with.
    ping_interval: Duration,

    /// [`WsSession`] closed reason. Should be set by the moment
    /// `Actor::stopped()` for this [`WsSession`] is called.
    close_reason: Option<InnerCloseReason>,

    /// [`SpawnHandle`] for the authentication checking task.
    auth_timeout_handle: Option<SpawnHandle>,
}

impl WsSession {
    /// Creates new [`WsSession`] for specified [`Member`].
    pub fn new(
        rooms: Box<dyn RpcServerRepository>,
        idle_timeout: Duration,
        ping_interval: Duration,
    ) -> Self {
        Self {
            id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            rpc_server_repo: rooms,
            sessions: HashMap::new(),
            idle_timeout,
            last_activity: Instant::now(),
            fragmentation_buffer: BytesMut::new(),
            last_ping_num: 0,
            ping_interval,
            close_reason: None,
            auth_timeout_handle: None,
        }
    }

    /// Handles text WebSocket messages.
    fn handle_text(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        text: &str,
    ) {
        self.last_activity = Instant::now();
        match serde_json::from_str::<ClientMsg>(&text) {
            Ok(ClientMsg::Pong(n)) => {
                debug!("{}: Received Pong: {}", self, n);
            }
            Ok(ClientMsg::Command { room_id, command }) => {
                debug!("{}: Received Command: {:?}", self, command);
                if let Some((member_id, room)) = self.sessions.get(&room_id) {
                    room.send_command(member_id.clone(), command);
                } else {
                    Self::send_left_room(ctx, room_id, CloseReason::Finished);
                    if self.sessions.is_empty() {
                        ctx.stop();
                    }
                }
            }
            Ok(ClientMsg::JoinRoom {
                room_id,
                member_id,
                credentials: token,
            }) => {
                self.handle_join_room(ctx, room_id, member_id, token);
            }
            Ok(ClientMsg::LeaveRoom { room_id, member_id }) => {
                self.handle_leave_room(
                    ctx,
                    &room_id,
                    member_id,
                    ClosedReason::Closed { normal: true },
                );
            }
            Err(err) => error!(
                "{}: Error [{:?}] parsing client message: [{}]",
                self, err, &text,
            ),
        }
    }

    /// Updates [`RpcConnectionSettings`] of this [`WsSession`].
    fn update_rpc_settings(&mut self, new_settings: RpcConnectionSettings) {
        if new_settings.idle_timeout < self.idle_timeout {
            self.idle_timeout = new_settings.idle_timeout;
        }
        if new_settings.ping_interval < self.ping_interval {
            self.ping_interval = new_settings.ping_interval;
        }
        // TODO: maybe we need to restart IDLE watchdog and pinger
    }

    /// Handler [`ClientMsg::JoinRoom`].
    fn handle_join_room(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        room_id: RoomId,
        member_id: MemberId,
        credentials: Credentials,
    ) {
        if let Some(room) = self.rpc_server_repo.get(&room_id) {
            room.connection_established(
                member_id.clone(),
                credentials,
                Box::new(ctx.address()),
            )
            .into_actor(self)
            .map(|result, this, ctx| match result {
                Ok(settings) => {
                    this.update_rpc_settings(settings);
                    this.sessions
                        .insert(room_id.clone(), (member_id.clone(), room));
                    this.auth_timeout_handle.take();
                    Self::send_join_room(ctx, room_id, member_id);
                }
                Err(_) => Self::send_left_room(
                    ctx,
                    room_id,
                    CloseReason::InternalError,
                ),
            })
            .wait(ctx);
        } else {
            Self::send_left_room(ctx, room_id, CloseReason::Rejected)
        }
    }

    /// Handles [`ClientMsg::LeftRoom`].
    fn handle_leave_room(
        &self,
        ctx: &mut ws::WebsocketContext<Self>,
        room_id: &RoomId,
        member_id: MemberId,
        reason: ClosedReason,
    ) {
        if let Some(room) = self.rpc_server_repo.get(room_id) {
            ctx.spawn(
                room.connection_closed(member_id, reason).into_actor(self),
            );
        }
    }

    /// Handles WebSocket close frame.
    fn handle_close(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        reason: Option<WsCloseReason>,
    ) {
        debug!("{}: Received Close message: {:?}", self, reason);
        if self.close_reason.is_none() {
            let closed_reason = if let Some(reason) = &reason {
                if reason.code == CloseCode::Normal
                    || reason.code == CloseCode::Away
                {
                    ClosedReason::Closed { normal: true }
                } else {
                    ClosedReason::Lost
                }
            } else {
                ClosedReason::Lost
            };

            self.close_reason = Some(InnerCloseReason::ByClient(closed_reason));
            ctx.close(reason);
            ctx.stop();
        }
    }

    /// Handles WebSocket continuation frame.
    fn handle_continuation(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        frame: Item,
    ) {
        // This is logged as at `WARN` level, because fragmentation usually
        // happens only when dealing with large payloads (>128kb in Chrome).
        // We will handle this message, but it probably signals that some
        // bug occurred on sending side.
        warn!("{}: Continuation frame received.", self);
        match frame {
            Item::FirstText(value) => {
                if !self.fragmentation_buffer.is_empty() {
                    error!(
                        "{}: Received new continuation frame before \
                         completing previous.",
                        self
                    );
                    self.fragmentation_buffer.clear();
                }
                self.fragmentation_buffer.extend_from_slice(value.bytes());
            }
            Item::FirstBinary(_) => {
                error!(
                    "{}: Received unexpected continuation-binary frame.",
                    self
                );
            }
            Item::Continue(value) => {
                if self.fragmentation_buffer.is_empty() {
                    error!(
                        "{}: Received continuation frame that was not \
                         preceded by continuation-first frame",
                        self
                    );
                } else {
                    self.fragmentation_buffer.extend_from_slice(value.bytes());
                }
            }
            Item::Last(value) => {
                self.fragmentation_buffer.extend_from_slice(value.bytes());
                let frame = self.fragmentation_buffer.split();
                match std::str::from_utf8(frame.as_ref()) {
                    Ok(text) => self.handle_text(ctx, &text),
                    Err(err) => {
                        error!("{}: Could not parse ws frame: {}", self, err);
                    }
                }
            }
        }
    }

    /// Sends close frame and stops connection [`Actor`].
    fn close_in_place(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        reason: &CloseDescription,
    ) {
        debug!("{}: Closing WsSession", self);
        self.close_reason = Some(InnerCloseReason::ByServer);
        ctx.close(Some(ws::CloseReason {
            code: ws::CloseCode::Normal,
            description: Some(serde_json::to_string(reason).unwrap()),
        }));
        ctx.stop();
    }

    /// Starts watchdog which will drop connection if `now`-`last_activity` >
    /// `idle_timeout`.
    fn start_idle_watchdog(ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(Duration::new(1, 0), |this, ctx| {
            if Instant::now().duration_since(this.last_activity)
                > this.idle_timeout
            {
                info!("{}: WsSession is idle", this);

                let session = std::mem::take(&mut this.sessions);
                let close_all_session =
                    session.into_iter().map(|(_, (member_id, room))| {
                        room.connection_closed(member_id, ClosedReason::Lost)
                    });
                Arbiter::spawn(
                    futures::future::join_all(close_all_session).map(|_| ()),
                );

                this.close_in_place(
                    ctx,
                    &CloseDescription::new(CloseReason::Idle),
                );
            }
        });
    }

    /// Sends [`ServerMsg::Ping`] immediately and starts ping send scheduler
    /// with `ping_interval`.
    fn start_heartbeat(&mut self, ctx: &mut <Self as Actor>::Context) {
        self.send_ping(ctx);
        ctx.run_interval(self.ping_interval, |this, ctx| {
            this.send_ping(ctx);
        });
    }

    /// Sends [`ServerMsg::Ping`] increasing ping counter.
    fn send_ping(&mut self, ctx: &mut <Self as Actor>::Context) {
        ctx.text(
            serde_json::to_string(&ServerMsg::Ping(self.last_ping_num))
                .unwrap(),
        );
        self.last_ping_num += 1;
    }

    /// Sends [`ServerMsg::JoinedRoom`] to the client.
    fn send_join_room(
        ctx: &mut <Self as Actor>::Context,
        room_id: RoomId,
        member_id: MemberId,
    ) {
        ctx.text(
            serde_json::to_string(&ServerMsg::JoinedRoom {
                room_id,
                member_id,
            })
            .unwrap(),
        );
    }

    /// Sends [`ServerMsg::LeftRoom`] to the client.
    fn send_left_room(
        ctx: &mut <Self as Actor>::Context,
        room_id: RoomId,
        close_reason: CloseReason,
    ) {
        ctx.text(
            serde_json::to_string(&ServerMsg::LeftRoom {
                room_id,
                close_reason,
            })
            .unwrap(),
        );
    }

    /// Sends current [`RpcSettings`] to the client.
    fn send_current_rpc_settings(&self, ctx: &mut <Self as Actor>::Context) {
        let rpc_settings = RpcSettings {
            idle_timeout_ms: self
                .idle_timeout
                .as_millis()
                .try_into()
                .expect("'idle_timeout' should fit into u64"),
            ping_interval_ms: self
                .ping_interval
                .as_millis()
                .try_into()
                .expect("'ping_interval' should fit into u64"),
        };
        ctx.text(
            serde_json::to_string(&ServerMsg::RpcSettings(rpc_settings))
                .unwrap(),
        );
    }
}

/// [`Actor`] implementation that provides an ergonomic way to deal with
/// WebSocket connection lifecycle for [`WsSession`].
impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    /// Starts [`Heartbeat`] mechanism and sends [`RpcConnectionEstablished`]
    /// signal to the [`Room`].
    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("{}: WsSession started", self);
        self.send_current_rpc_settings(ctx);
        self.start_heartbeat(ctx);
        Self::start_idle_watchdog(ctx);
        let auth_timeout_task = ctx.run_later(AUTH_TIMEOUT, |this, ctx| {
            info!("{}: WsSession is idle", this);
            if this.sessions.is_empty() {
                this.close_in_place(
                    ctx,
                    &CloseDescription::new(CloseReason::Rejected),
                );
            }
        });
        self.auth_timeout_handle.replace(auth_timeout_task);
    }

    /// Invokes `RpcServer::connection_closed()` with `ClosedReason::Lost` if
    /// `WsSession.close_reason` is `None`, with [`ClosedReason`] defined in
    /// `WsSession.close_reason` if it is `Some(InnerCloseReason::ByClient)`,
    /// does nothing if `WsSession.close_reason` is
    /// `Some(InnerCloseReason::ByServer)`.
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("{}: WsSession Stopped", self);
        let session = std::mem::take(&mut self.sessions);
        let reason = match self.close_reason.take() {
            None => {
                error!("{}: WsSession was unexpectedly dropped", self);
                ClosedReason::Lost
            }
            Some(InnerCloseReason::ByClient(reason)) => reason,
            Some(InnerCloseReason::ByServer) => {
                return;
            }
        };
        let close_all_session =
            session.into_iter().map(|(_, (member_id, room))| {
                room.connection_closed(member_id, reason)
            });
        Arbiter::spawn(
            futures::future::join_all(close_all_session).map(|_| ()),
        );
    }
}

impl RpcConnection for Addr<WsSession> {
    /// Closes [`WsSession`] by sending itself "normal closure" close message
    /// with [`CloseDescription`] as description of [Close] frame.
    ///
    /// [Close]:https://tools.ietf.org/html/rfc6455#section-5.5.1
    fn close(
        &mut self,
        room_id: RoomId,
        close_description: CloseDescription,
    ) -> LocalBoxFuture<'static, ()> {
        let close_result = self.send(CloseRoom {
            room_id,
            close_description,
        });
        async {
            if let Err(err) = close_result.await {
                match err {
                    MailboxError::Closed => {
                        // RpcConnection is already closed, so it ok
                    }
                    MailboxError::Timeout => {
                        error!("Failed Close RpcConnection")
                    }
                }
            }
        }
        .boxed_local()
    }

    /// Sends [`Event`] to Web Client.
    ///
    /// [`Event`]: medea_client_api_proto::Event
    fn send_event(&self, room_id: RoomId, event: Event) {
        self.do_send(EventMessage { room_id, event });
    }
}

/// Message which indicates that [`WsSession`] should close connection for the
/// provided [`RoomId`] with provided [`CloseDescription`] as close reason.
#[derive(Message)]
#[rtype(result = "()")]
pub struct CloseRoom {
    /// [`RoomId`] of [`Room`] which should be closed.
    room_id: RoomId,

    /// [`CloseDescription`] with which this [`Room`] should be closed.
    close_description: CloseDescription,
}

impl Handler<CloseRoom> for WsSession {
    type Result = ();

    fn handle(
        &mut self,
        msg: CloseRoom,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if self.sessions.remove(&msg.room_id).is_some() {
            Self::send_left_room(ctx, msg.room_id, CloseReason::Finished);
            if self.sessions.is_empty() {
                self.close_in_place(ctx, &msg.close_description);
            }
        }
    }
}

impl Handler<EventMessage> for WsSession {
    type Result = ();

    /// Sends [`Event`] to Web Client.
    fn handle(&mut self, msg: EventMessage, ctx: &mut Self::Context) {
        debug!("{}: Sending Event: {:?}]", self, msg);
        let event = serde_json::to_string(&ServerMsg::Event {
            room_id: msg.room_id,
            event: msg.event,
        })
        .unwrap();
        ctx.text(event);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    /// Handles arbitrary [`ws::Message`] received from WebSocket client.
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(msg) => match msg {
                ws::Message::Text(text) => self.handle_text(ctx, &text),
                ws::Message::Close(reason) => self.handle_close(ctx, reason),
                ws::Message::Continuation(item) => {
                    self.handle_continuation(ctx, item);
                }
                ws::Message::Binary(_) => {
                    warn!("{}: Received binary message", self);
                }
                ws::Message::Ping(ping) => {
                    ctx.pong(ping.bytes());
                }
                ws::Message::Nop | ws::Message::Pong(_) => {
                    // nothing to do here
                }
            },
            Err(err) => {
                error!("{}: StreamHandler Error: {:?}", self, err);
                self.close_in_place(
                    ctx,
                    &CloseDescription {
                        reason: CloseReason::InternalError,
                    },
                );
            }
        };
    }

    /// Method is called when stream finishes. Stops [`WsSession`] actor
    /// execution.
    fn finished(&mut self, ctx: &mut Self::Context) {
        debug!("{}: message stream is finished", self);
        ctx.stop()
    }
}

impl Display for WsSession {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "WsSession [{}]", self.id)
    }
}

#[cfg(test)]
mod test {
    use std::{
        sync::Mutex,
        time::{Duration, Instant},
    };

    use actix_http::ws::Item;
    use actix_web::{test::TestServer, web, App, HttpRequest};
    use actix_web_actors::ws::{start, CloseCode, CloseReason, Frame, Message};
    use bytes::{Buf, Bytes};
    use futures::{
        channel::{
            mpsc::{self, UnboundedReceiver, UnboundedSender},
            oneshot::{self, Receiver, Sender},
        },
        future, FutureExt as _, SinkExt as _, StreamExt as _,
    };
    use medea_client_api_proto::{
        ClientMsg, CloseDescription, CloseReason as ProtoCloseReason, Command,
        Event, IceCandidate, MemberId, PeerId, RpcSettings, ServerMsg,
    };
    use tokio::time::timeout;

    use crate::api::{
        client::rpc_connection::{
            ClosedReason, RpcConnection, RpcConnectionSettings,
        },
        MockRpcServer,
    };

    use super::{MockRpcServerRepository, WsSession};

    type SharedOneshot<T> =
        (Mutex<Option<Sender<T>>>, Mutex<Option<Receiver<T>>>);
    type SharedUnbounded<T> = (
        Mutex<UnboundedSender<T>>,
        Mutex<Option<UnboundedReceiver<T>>>,
    );

    fn test_server(factory: fn() -> WsSession) -> TestServer {
        actix_web::test::start(move || {
            App::new().service(web::resource("/").to(
                move |req: HttpRequest, stream: web::Payload| async move {
                    start(factory(), &req, stream)
                },
            ))
        })
    }

    // WsSession is dropped and WebSocket connection is closed when RpcServer
    // errors on RpcConnectionEstablished.
    #[actix_rt::test]
    async fn close_if_rpc_established_failed() {
        fn factory() -> WsSession {
            let mut rpc_server_repo = MockRpcServerRepository::new();
            rpc_server_repo.expect_get().returning(|_| {
                let member_id = MemberId::from("member_id");
                let mut rpc_server = MockRpcServer::new();

                let expected_member_id = member_id.clone();
                rpc_server
                    .expect_connection_established()
                    .withf(move |member_id, _, _| {
                        *member_id == expected_member_id
                    })
                    .return_once(|_, _, _| future::err(()).boxed_local());
                rpc_server
                    .expect_connection_closed()
                    .returning(|_, _| future::ready(()).boxed_local());

                Some(Box::new(rpc_server))
            });

            WsSession::new(
                Box::new(rpc_server_repo),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        }

        let mut serv = test_server(factory);

        let mut client = serv.ws().await.unwrap();

        let join_msg = ClientMsg::JoinRoom {
            room_id: "room_id".into(),
            member_id: "member_id".into(),
            credentials: "token".into(),
        };
        client
            .send(Message::Text(
                std::str::from_utf8(
                    Bytes::from(serde_json::to_string(&join_msg).unwrap())
                        .bytes(),
                )
                .unwrap()
                .to_owned(),
            ))
            .await
            .unwrap();

        let mut client = client.skip(2);
        let left_room_frame = client.next().await.unwrap().unwrap();
        let expected_left_room_frame = Frame::Text(
            serde_json::to_string(&ServerMsg::LeftRoom {
                room_id: "room_id".into(),
                close_reason:
                    medea_client_api_proto::CloseReason::InternalError,
            })
            .unwrap()
            .into(),
        );
        assert_eq!(left_room_frame, expected_left_room_frame);

        let item = client.next().await.unwrap().unwrap();
        let close_frame = Frame::Close(Some(CloseReason {
            code: CloseCode::Normal,
            description: Some(String::from(r#"{"reason":"Rejected"}"#)),
        }));
        assert_eq!(item, close_frame);
    }

    #[actix_rt::test]
    async fn sends_rpc_settings_and_pings() {
        let mut serv = test_server(|| -> WsSession {
            let mut rpc_server_repo = MockRpcServerRepository::new();
            rpc_server_repo.expect_get().returning(|_| {
                let mut rpc_server = MockRpcServer::new();

                rpc_server.expect_connection_established().return_once(
                    |_, _, _| {
                        future::ok(RpcConnectionSettings {
                            ping_interval: Duration::from_secs(10),
                            idle_timeout: Duration::from_secs(10),
                        })
                        .boxed_local()
                    },
                );
                rpc_server
                    .expect_connection_closed()
                    .returning(|_, _| future::ready(()).boxed_local());

                Some(Box::new(rpc_server))
            });

            WsSession::new(
                Box::new(rpc_server_repo),
                Duration::from_secs(5),
                Duration::from_millis(50),
            )
        });

        let mut client = serv.ws().await.unwrap();

        let join_msg = ClientMsg::JoinRoom {
            room_id: "room_id".into(),
            member_id: "member_id".into(),
            credentials: "token".into(),
        };
        client
            .send(Message::Text(
                std::str::from_utf8(
                    Bytes::from(serde_json::to_string(&join_msg).unwrap())
                        .bytes(),
                )
                .unwrap()
                .to_owned(),
            ))
            .await
            .unwrap();

        fn msg_to_text_frame(msg: &ServerMsg) -> Frame {
            Frame::Text(serde_json::to_string(msg).unwrap().into())
        }

        // let mut client = client;
        let item = client.next().await.unwrap().unwrap();
        let expected_item =
            msg_to_text_frame(&ServerMsg::RpcSettings(RpcSettings {
                idle_timeout_ms: 5000,
                ping_interval_ms: 50,
            }));
        assert_eq!(item, expected_item);

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(item, msg_to_text_frame(&ServerMsg::Ping(0)));

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(
            item,
            msg_to_text_frame(&ServerMsg::JoinedRoom {
                room_id: "room_id".into(),
                member_id: "member_id".into(),
            })
        );

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(item, msg_to_text_frame(&ServerMsg::Ping(1)));
    }

    // WsSession is dropped and WebSocket connection is closed if no pongs
    // received for idle_timeout.
    #[actix_rt::test]
    async fn dropped_if_idle() {
        let mut serv = test_server(|| -> WsSession {
            let mut rpc_server_repo = MockRpcServerRepository::new();
            rpc_server_repo.expect_get().returning(|_| {
                let expected_member_id = MemberId::from("member_id");
                let mut rpc_server = MockRpcServer::new();

                rpc_server.expect_connection_established().return_once(
                    |_, _, _| {
                        future::ok(RpcConnectionSettings {
                            ping_interval: Duration::from_secs(10),
                            idle_timeout: Duration::from_secs(10),
                        })
                        .boxed_local()
                    },
                );

                rpc_server
                    .expect_connection_closed()
                    .withf(move |member_id, reason| {
                        *member_id == expected_member_id
                            && *reason == ClosedReason::Lost
                    })
                    .return_once(|_, _| future::ready(()).boxed_local());

                Some(Box::new(rpc_server))
            });

            WsSession::new(
                Box::new(rpc_server_repo),
                Duration::from_millis(100),
                Duration::from_secs(10),
            )
        });

        let mut client = serv.ws().await.unwrap();

        let join_msg = ClientMsg::JoinRoom {
            room_id: "room_id".into(),
            member_id: "member_id".into(),
            credentials: "token".into(),
        };
        client
            .send(Message::Text(
                std::str::from_utf8(
                    Bytes::from(serde_json::to_string(&join_msg).unwrap())
                        .bytes(),
                )
                .unwrap()
                .to_owned(),
            ))
            .await
            .unwrap();

        let start = std::time::Instant::now();

        let item = client.skip(3).next().await.unwrap().unwrap();

        let close_frame = Frame::Close(Some(CloseReason {
            code: CloseCode::Normal,
            description: Some(String::from(r#"{"reason":"Idle"}"#)),
        }));
        assert_eq!(item, close_frame);

        assert!(
            Instant::now().duration_since(start) > Duration::from_millis(99)
        );
        assert!(Instant::now().duration_since(start) < Duration::from_secs(2));
    }

    // Make sure that WsSession redirects all Commands it receives to
    #[actix_rt::test]
    async fn passes_commands_to_rpc_server() {
        lazy_static::lazy_static! {
            static ref CHAN: SharedUnbounded<Command> = {
                let (tx, rx) = mpsc::unbounded();
                (Mutex::new(tx), Mutex::new(Some(rx)))
            };
        }

        let mut serv = test_server(|| -> WsSession {
            let mut rpc_server_repo = MockRpcServerRepository::new();
            rpc_server_repo.expect_get().returning(|_| {
                let mut rpc_server = MockRpcServer::new();

                rpc_server.expect_connection_established().return_once(
                    |_, _, _| {
                        future::ok(RpcConnectionSettings {
                            idle_timeout: Duration::from_secs(10),
                            ping_interval: Duration::from_secs(10),
                        })
                        .boxed_local()
                    },
                );
                rpc_server
                    .expect_connection_closed()
                    .returning(|_, _| future::ready(()).boxed_local());

                rpc_server.expect_send_command().returning(|_, command| {
                    CHAN.0.lock().unwrap().unbounded_send(command).unwrap();
                });

                Some(Box::new(rpc_server))
            });

            WsSession::new(
                Box::new(rpc_server_repo),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        });

        let mut client = serv.ws().await.unwrap();

        let join_msg = ClientMsg::JoinRoom {
            room_id: "room_id".into(),
            member_id: "member_id".into(),
            credentials: "token".into(),
        };
        client
            .send(Message::Text(
                std::str::from_utf8(
                    Bytes::from(serde_json::to_string(&join_msg).unwrap())
                        .bytes(),
                )
                .unwrap()
                .to_owned(),
            ))
            .await
            .unwrap();

        let cmd = ClientMsg::Command {
            room_id: "room_id".into(),
            command: Command::SetIceCandidate {
                peer_id: PeerId(15),
                candidate: IceCandidate {
                    candidate: "asd".to_string(),
                    sdp_m_line_index: Some(1),
                    sdp_mid: Some("2".to_string()),
                },
            },
        };
        let command = Bytes::from(serde_json::to_string(&cmd).unwrap());

        client
            .send(Message::Text(
                std::str::from_utf8(command.bytes()).unwrap().to_owned(),
            ))
            .await
            .unwrap();
        client
            .send(Message::Continuation(Item::FirstText(command.slice(0..10))))
            .await
            .unwrap();
        client
            .send(Message::Continuation(Item::Last(
                command.slice(10..command.len()),
            )))
            .await
            .unwrap();
        client
            .send(Message::Continuation(Item::FirstText(command.slice(0..10))))
            .await
            .unwrap();
        client
            .send(Message::Continuation(Item::Continue(command.slice(10..20))))
            .await
            .unwrap();
        client
            .send(Message::Continuation(Item::Last(
                command.slice(20..command.len()),
            )))
            .await
            .unwrap();

        let commands: Vec<Command> = timeout(
            Duration::from_millis(500),
            CHAN.1.lock().unwrap().take().unwrap().take(3).collect(),
        )
        .await
        .unwrap();
        for command in commands {
            match command {
                Command::SetIceCandidate { peer_id, candidate } => {
                    assert_eq!(peer_id.0, 15);
                    assert_eq!(candidate.candidate, "asd");
                }
                _ => unreachable!(),
            }
        }
    }

    // WsSession is dropped and WebSocket connection is closed when
    // RpcConnection::close is called.
    #[actix_rt::test]
    async fn close_when_rpc_connection_close() {
        lazy_static::lazy_static! {
            static ref CHAN: SharedOneshot<Box<dyn RpcConnection>> = {
                let (tx, rx) = oneshot::channel();
                (Mutex::new(Some(tx)), Mutex::new(Some(rx)))
            };
        }

        let mut serv = test_server(|| -> WsSession {
            let mut rpc_server_repo = MockRpcServerRepository::new();
            rpc_server_repo.expect_get().returning(|_| {
                let mut rpc_server = MockRpcServer::new();

                rpc_server.expect_connection_established().return_once(
                    |_, _, connection| {
                        let _ = CHAN
                            .0
                            .lock()
                            .unwrap()
                            .take()
                            .unwrap()
                            .send(connection);
                        future::ok(RpcConnectionSettings {
                            idle_timeout: Duration::from_secs(10),
                            ping_interval: Duration::from_secs(10),
                        })
                        .boxed_local()
                    },
                );
                rpc_server
                    .expect_connection_closed()
                    .returning(|_, _| future::ready(()).boxed_local());

                Some(Box::new(rpc_server))
            });

            WsSession::new(
                Box::new(rpc_server_repo),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        });

        let mut client = serv.ws().await.unwrap();

        let join_msg = ClientMsg::JoinRoom {
            room_id: "room_id".into(),
            member_id: "member_id".into(),
            credentials: "token".into(),
        };
        client
            .send(Message::Text(
                std::str::from_utf8(
                    Bytes::from(serde_json::to_string(&join_msg).unwrap())
                        .bytes(),
                )
                .unwrap()
                .to_owned(),
            ))
            .await
            .unwrap();

        let mut rpc_connection: Box<dyn RpcConnection> =
            CHAN.1.lock().unwrap().take().unwrap().await.unwrap();

        rpc_connection
            .close(
                "room_id".into(),
                CloseDescription {
                    reason: ProtoCloseReason::Evicted,
                },
            )
            .await;
        let mut client = client.skip(3);

        let left_room_frame = client.next().await.unwrap().unwrap();
        let correct_left_room_frame = Frame::Text(Bytes::from(
            serde_json::to_string(&ServerMsg::LeftRoom {
                room_id: "room_id".into(),
                close_reason: medea_client_api_proto::CloseReason::Finished,
            })
            .unwrap(),
        ));
        assert_eq!(left_room_frame, correct_left_room_frame);
        let item = client.next().await.unwrap().unwrap();

        let close_frame = Frame::Close(Some(CloseReason {
            code: CloseCode::Normal,
            description: Some(String::from(r#"{"reason":"Evicted"}"#)),
        }));

        assert_eq!(item, close_frame);
    }

    // WsSession transmits Events to WebSocket client when
    // RpcConnection::send_event is called.
    #[actix_rt::test]
    async fn send_text_message_when_rpc_connection_send_event() {
        lazy_static::lazy_static! {
            static ref CHAN: SharedOneshot<Box<dyn RpcConnection>> = {
                let (tx, rx) = oneshot::channel();
                (Mutex::new(Some(tx)), Mutex::new(Some(rx)))
            };
        }

        let mut serv = test_server(|| -> WsSession {
            let mut rpc_server_repo = MockRpcServerRepository::new();
            rpc_server_repo.expect_get().returning(|_| {
                let mut rpc_server = MockRpcServer::new();

                rpc_server.expect_connection_established().return_once(
                    |_, _, connection| {
                        let _ = CHAN
                            .0
                            .lock()
                            .unwrap()
                            .take()
                            .unwrap()
                            .send(connection);
                        future::ok(RpcConnectionSettings {
                            ping_interval: Duration::from_secs(10),
                            idle_timeout: Duration::from_secs(10),
                        })
                        .boxed_local()
                    },
                );
                rpc_server
                    .expect_connection_closed()
                    .returning(|_, _| future::ready(()).boxed_local());

                Some(Box::new(rpc_server))
            });

            WsSession::new(
                Box::new(rpc_server_repo),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        });

        let mut client = serv.ws().await.unwrap();
        let join_msg = ClientMsg::JoinRoom {
            room_id: "room_id".into(),
            member_id: "member_id".into(),
            credentials: "token".into(),
        };
        client
            .send(Message::Text(
                std::str::from_utf8(
                    Bytes::from(serde_json::to_string(&join_msg).unwrap())
                        .bytes(),
                )
                .unwrap()
                .to_owned(),
            ))
            .await
            .unwrap();

        let rpc_connection: Box<dyn RpcConnection> =
            CHAN.1.lock().unwrap().take().unwrap().await.unwrap();

        rpc_connection.send_event(
            "room_id".into(),
            Event::SdpAnswerMade {
                peer_id: PeerId(77),
                sdp_answer: String::from("sdp_answer"),
            },
        );

        let item = client.skip(3).next().await.unwrap().unwrap();

        let event = serde_json::to_string(&ServerMsg::Event {
            room_id: "room_id".into(),
            event: Event::SdpAnswerMade {
                peer_id: PeerId(77),
                sdp_answer: "sdp_answer".to_string(),
            },
        })
        .unwrap();

        assert_eq!(item, Frame::Text(event.into()));
    }
}
