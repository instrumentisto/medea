//! WebSocket session.

use std::{
    collections::HashMap,
    convert::TryInto as _,
    fmt::{Debug, Display, Error, Formatter},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use actix::{
    Actor, ActorContext, ActorFutureExt as _, Addr, AsyncContext,
    ContextFutureSpawner as _, Handler, MailboxError, Message, SpawnHandle,
    StreamHandler, WrapFuture,
};
use actix_http::ws::{CloseReason as WsCloseReason, Item};
use actix_web_actors::ws::{self, CloseCode};
use bytes::BytesMut;
use futures::future::{self, FutureExt as _, LocalBoxFuture};
use medea_client_api_proto::{
    state, ClientMsg, CloseDescription, CloseReason, Command, Credential,
    Event, MemberId, RoomId, RpcSettings, ServerMsg,
};

use crate::{
    api::{
        client::rpc_connection::{
            ClosedReason, EventMessage, RpcConnection, RpcConnectionSettings,
        },
        RpcServer, RpcServerError,
    },
    log::prelude::*,
};

use super::MAX_WS_MSG_SIZE;

/// Repository of the all [`RpcServer`]s registered on this Media Server.
#[cfg_attr(test, mockall::automock)]
pub trait RpcServerRepository: Debug {
    /// Returns [`RpcServer`] with a provided [`RoomId`].
    ///
    /// Returns `None` if [`RpcServer`] with a provided [`RoomId`] doesn't
    /// exists.
    fn get(&self, room_id: &RoomId) -> Option<Box<dyn RpcServer>>;
}

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
    /// idle (no messages received during [`WsSession::idle_timeout`]).
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

    /// [`SpawnHandle`] for the heartbeat task.
    heartbeat_handle: Option<SpawnHandle>,
}

impl WsSession {
    /// Creates new [`WsSession`] for specified [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
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
            heartbeat_handle: None,
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
                match command {
                    Command::JoinRoom {
                        member_id,
                        credential,
                    } => {
                        self.handle_join_room(
                            ctx, room_id, member_id, credential,
                        );
                    }
                    Command::LeaveRoom { member_id } => {
                        self.handle_leave_room(
                            ctx,
                            &room_id,
                            member_id,
                            ClosedReason::Closed { normal: true },
                        );
                    }
                    Command::SynchronizeMe { state } => {
                        self.handle_synchronize_me(ctx, &room_id, &state);
                    }
                    _ => {
                        if let Some((member_id, room)) =
                            self.sessions.get(&room_id)
                        {
                            room.send_command(member_id.clone(), command);
                        } else {
                            self.send_left_room(
                                ctx,
                                room_id,
                                CloseReason::Finished,
                            );
                            if self.sessions.is_empty() {
                                self.close_in_place(
                                    ctx,
                                    &CloseDescription::new(
                                        CloseReason::Rejected,
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            Err(err) => error!(
                "{}: Error [{:?}] parsing client message: [{}]",
                self, err, &text,
            ),
        }
    }

    /// Updates [`RpcConnectionSettings`] of this [`WsSession`].
    ///
    /// Updates will be performed only if old settings are less then new one.
    ///
    /// Sends [`ServerMsg::RpcSettings`] to the client if some settings was
    /// updated.
    ///
    /// Restarts heartbeater with a new [`RpcConnectionSettings`].
    fn update_rpc_settings(
        &mut self,
        new_settings: RpcConnectionSettings,
        ctx: &mut ws::WebsocketContext<Self>,
    ) {
        let mut updated = false;
        if new_settings.idle_timeout < self.idle_timeout {
            self.idle_timeout = new_settings.idle_timeout;
            updated = true;
        }
        if new_settings.ping_interval < self.ping_interval {
            self.ping_interval = new_settings.ping_interval;
            updated = true;
        }
        if updated {
            self.send_current_rpc_settings(ctx);
            self.start_heartbeat(ctx);
        }
    }

    /// Handler for [`Command::JoinRoom`].
    ///
    /// Calls [`RpcServer::connection_established`], updates
    /// [`RpcConnectionSettings`] with [`RpcConnectionSettings`] returned from
    /// the [`RpcServer`].
    ///
    /// Sends [`Event::RoomJoined`].
    fn handle_join_room(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        room_id: RoomId,
        member_id: MemberId,
        credential: Credential,
    ) {
        if let Some(room) = self.rpc_server_repo.get(&room_id) {
            room.connection_established(
                member_id.clone(),
                credential,
                Box::new(ctx.address()),
            )
            .into_actor(self)
            .map(|result, this, ctx| match result {
                Ok(settings) => {
                    this.update_rpc_settings(settings, ctx);
                    this.sessions
                        .insert(room_id.clone(), (member_id.clone(), room));
                    if let Some(auth_timeout_handle) =
                        this.auth_timeout_handle.take()
                    {
                        ctx.cancel_future(auth_timeout_handle);
                    }
                    this.send_joined_room(ctx, room_id, member_id);
                }
                Err(err) => {
                    error!(
                        "{}: Failed to authorize Rpc Session `{}/{}` cause: \
                         {:?}",
                        this, room_id, member_id, err
                    );
                    let reason = match err {
                        RpcServerError::Authorization => CloseReason::Rejected,
                        RpcServerError::RoomError(_)
                        | RpcServerError::RoomMailbox(_) => {
                            CloseReason::InternalError
                        }
                    };
                    this.send_left_room(ctx, room_id, reason)
                }
            })
            .wait(ctx);
        } else {
            error!(
                "{}: Failed to authorize Rpc Session: Room `{}` does not exist",
                self, room_id
            );
            self.send_left_room(ctx, room_id, CloseReason::Rejected)
        }
    }

    /// Handles [`Command::LeaveRoom`].
    ///
    /// Sends [`RpcServer::connection_closed`] to the [`RpcServer`] based on
    /// provided [`RoomId`].
    fn handle_leave_room(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        room_id: &RoomId,
        _: MemberId,
        reason: ClosedReason,
    ) {
        if let Some((member, room)) = self.sessions.remove(&room_id) {
            actix::spawn(room.connection_closed(member, reason));
        }
        if self.sessions.is_empty() {
            self.close_in_place(
                ctx,
                &CloseDescription::new(CloseReason::Finished),
            )
        }
    }

    /// Handles [`Command::SynchronizeMe`].
    ///
    /// Sends [`RpcServer::synchronize`] to the [`RpcServer`] and locks
    /// [`WsSession`] event loop until this message is processed.
    fn handle_synchronize_me(
        &mut self,
        ctx: &mut ws::WebsocketContext<Self>,
        room_id: &RoomId,
        state: &state::Room,
    ) {
        debug!("{}: Received synchronization request: {:?}", self, state);
        if let Some((member_id, room)) = self.sessions.get(&room_id) {
            ctx.wait(room.synchronize(member_id.clone()).into_actor(self));
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
            let closed_reason =
                reason.as_ref().map_or(ClosedReason::Lost, |reason| {
                    if reason.code == CloseCode::Normal
                        || reason.code == CloseCode::Away
                    {
                        ClosedReason::Closed { normal: true }
                    } else {
                        ClosedReason::Lost
                    }
                });

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
        if let Item::Continue(value) | Item::Last(value) = &frame {
            if (self.fragmentation_buffer.len() + value.len()) > MAX_WS_MSG_SIZE
            {
                error!("{}: Fragmentation buffer overflow.", self);
                self.close_in_place(
                    ctx,
                    &CloseDescription::new(CloseReason::Evicted),
                );
                return;
            }
        }

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
                self.fragmentation_buffer.extend_from_slice(&*value);
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
                    self.fragmentation_buffer.extend_from_slice(&*value);
                }
            }
            Item::Last(value) => {
                self.fragmentation_buffer.extend_from_slice(&*value);
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
        if let Some(heartbeat_handle) = self.heartbeat_handle.take() {
            ctx.cancel_future(heartbeat_handle);
        } else {
            self.send_ping(ctx);
        }
        self.heartbeat_handle =
            Some(ctx.run_interval(self.ping_interval, |this, ctx| {
                this.send_ping(ctx);
            }));
    }

    /// Sends [`ServerMsg::Ping`] increasing ping counter.
    fn send_ping(&mut self, ctx: &mut <Self as Actor>::Context) {
        ctx.text(
            serde_json::to_string(&ServerMsg::Ping(self.last_ping_num))
                .unwrap(),
        );
        self.last_ping_num += 1;
    }

    /// Sends [`Event`] to Web Client.
    fn send_event(
        &self,
        ctx: &mut <Self as Actor>::Context,
        room_id: RoomId,
        event: Event,
    ) {
        debug!(
            "{}: Sending Event for Room [id = {}]: {:?}]",
            self, room_id, event
        );
        let event = serde_json::to_string(&ServerMsg::Event { room_id, event })
            .unwrap();
        ctx.text(event);
    }

    /// Sends [`Event::RoomJoined`] to the client.
    fn send_joined_room(
        &self,
        ctx: &mut <Self as Actor>::Context,
        room_id: RoomId,
        member_id: MemberId,
    ) {
        self.send_event(ctx, room_id, Event::RoomJoined { member_id });
    }

    /// Sends [`Event::RoomLeft`] to the client.
    fn send_left_room(
        &self,
        ctx: &mut <Self as Actor>::Context,
        room_id: RoomId,
        close_reason: CloseReason,
    ) {
        self.send_event(ctx, room_id, Event::RoomLeft { close_reason });
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

    /// Sends default [`RpcSettings`], starts heartbeat, idle watchdog and
    /// authentication timeout watchdog.
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
        actix::spawn(future::join_all(close_all_session).map(drop));
    }
}

impl RpcConnection for Addr<WsSession> {
    /// Closes [`RpcConnection`] by sending itself "normal closure" close
    /// message with [`CloseDescription`] as description of [Close] frame.
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
    ///
    /// [`Room`]: crate::signalling::room::Room
    room_id: RoomId,

    /// [`CloseDescription`] with which this [`Room`] should be closed.
    ///
    /// [`Room`]: crate::signalling::room::Room
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
            self.send_left_room(ctx, msg.room_id, msg.close_description.reason);
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
        self.send_event(ctx, msg.room_id, msg.event);
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
                ws::Message::Ping(ping) => ctx.pong(&*ping),
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
        str,
        sync::Mutex,
        time::{Duration, Instant},
    };

    use actix_http::{ws::Item, HttpService};
    use actix_http_test::TestServer;
    use actix_service::map_config;
    use actix_web::{dev::AppConfig, web, App, HttpRequest};
    use actix_web_actors::ws::{start, CloseCode, CloseReason, Frame, Message};
    use bytes::Bytes;
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
        MockRpcServer, RpcServerError,
    };

    use super::{MockRpcServerRepository, WsSession};

    type SharedOneshot<T> =
        (Mutex<Option<Sender<T>>>, Mutex<Option<Receiver<T>>>);
    type SharedUnbounded<T> = (
        Mutex<UnboundedSender<T>>,
        Mutex<Option<UnboundedReceiver<T>>>,
    );

    fn into_frame(msg: ServerMsg) -> Frame {
        Frame::Text(serde_json::to_string(&msg).unwrap().into())
    }

    fn into_message(msg: ClientMsg) -> Message {
        Message::Text(
            str::from_utf8(&serde_json::to_vec(&msg).unwrap())
                .unwrap()
                .into(),
        )
    }

    async fn test_server(factory: fn() -> WsSession) -> TestServer {
        actix_http_test::test_server(move || {
            HttpService::new(map_config(
                App::new().service(web::resource("/").to(
                    move |req: HttpRequest, stream: web::Payload| async move {
                        start(factory(), &req, stream)
                    },
                )),
                |_| AppConfig::default(),
            ))
            .tcp()
        })
        .await
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
                    .return_once(|_, _, _| {
                        future::err(RpcServerError::Authorization).boxed_local()
                    });
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

        let mut serv = test_server(factory).await;

        let mut client = serv.ws().await.unwrap();

        client
            .send(into_message(ClientMsg::Command {
                room_id: "room_id".into(),
                command: Command::JoinRoom {
                    member_id: "member_id".into(),
                    credential: "token".into(),
                },
            }))
            .await
            .unwrap();

        let mut client = client.skip(2);
        let left_room_frame = client.next().await.unwrap().unwrap();

        assert_eq!(
            left_room_frame,
            into_frame(ServerMsg::Event {
                room_id: "room_id".into(),
                event: Event::RoomLeft {
                    close_reason: medea_client_api_proto::CloseReason::Rejected,
                }
            })
        );

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
        })
        .await;

        let mut client = serv.ws().await.unwrap();

        client
            .send(into_message(ClientMsg::Command {
                room_id: "room_id".into(),
                command: Command::JoinRoom {
                    member_id: "member_id".into(),
                    credential: "token".into(),
                },
            }))
            .await
            .unwrap();

        // let mut client = client;
        let item = client.next().await.unwrap().unwrap();
        let expected_item = into_frame(ServerMsg::RpcSettings(RpcSettings {
            idle_timeout_ms: 5000,
            ping_interval_ms: 50,
        }));
        assert_eq!(item, expected_item);

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(item, into_frame(ServerMsg::Ping(0)));

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(
            item,
            into_frame(ServerMsg::Event {
                room_id: "room_id".into(),
                event: Event::RoomJoined {
                    member_id: "member_id".into(),
                }
            })
        );

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(item, into_frame(ServerMsg::Ping(1)));
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
        })
        .await;

        let mut client = serv.ws().await.unwrap();

        client
            .send(into_message(ClientMsg::Command {
                room_id: "room_id".into(),
                command: Command::JoinRoom {
                    member_id: "member_id".into(),
                    credential: "token".into(),
                },
            }))
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
        })
        .await;

        let mut client = serv.ws().await.unwrap();

        client
            .send(into_message(ClientMsg::Command {
                room_id: "room_id".into(),
                command: Command::JoinRoom {
                    member_id: "member_id".into(),
                    credential: "token".into(),
                },
            }))
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

        client.send(into_message(cmd)).await.unwrap();
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
        })
        .await;

        let mut client = serv.ws().await.unwrap();

        client
            .send(into_message(ClientMsg::Command {
                room_id: "room_id".into(),
                command: Command::JoinRoom {
                    member_id: "member_id".into(),
                    credential: "token".into(),
                },
            }))
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
        assert_eq!(
            left_room_frame,
            into_frame(ServerMsg::Event {
                room_id: "room_id".into(),
                event: Event::RoomLeft {
                    close_reason: medea_client_api_proto::CloseReason::Evicted,
                }
            })
        );
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
        })
        .await;

        let mut client = serv.ws().await.unwrap();
        client
            .send(into_message(ClientMsg::Command {
                room_id: "room_id".into(),
                command: Command::JoinRoom {
                    member_id: "member_id".into(),
                    credential: "token".into(),
                },
            }))
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

    #[actix_rt::test]
    async fn multi_room_support() {
        lazy_static::lazy_static! {
            static ref CHAN: SharedUnbounded<Box<dyn RpcConnection>> = {
                let (tx, rx) = mpsc::unbounded();
                (Mutex::new(tx), Mutex::new(Some(rx)))
            };
        }

        let mut serv = test_server(|| -> WsSession {
            let mut rpc_server_repo = MockRpcServerRepository::new();
            rpc_server_repo.expect_get().returning(|_| {
                let mut rpc_server = MockRpcServer::new();

                rpc_server.expect_connection_established().return_once(
                    |_, _, connection| {
                        let _ =
                            CHAN.0.lock().unwrap().unbounded_send(connection);
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
        })
        .await;

        let mut client = serv.ws().await.unwrap();
        client
            .send(into_message(ClientMsg::Command {
                room_id: "alice_room".into(),
                command: Command::JoinRoom {
                    member_id: "alice".into(),
                    credential: "token".into(),
                },
            }))
            .await
            .unwrap();

        let mut connections_rx = CHAN.1.lock().unwrap().take().unwrap();
        let alice_connection: Box<dyn RpcConnection> =
            connections_rx.next().await.unwrap();

        let alice_event = Event::SdpAnswerMade {
            peer_id: PeerId(0),
            sdp_answer: String::from("sdp_answer"),
        };
        alice_connection.send_event("alice_room".into(), alice_event.clone());

        client
            .send(into_message(ClientMsg::Command {
                room_id: "bob_room".into(),
                command: Command::JoinRoom {
                    member_id: "bob".into(),
                    credential: "token".into(),
                },
            }))
            .await
            .unwrap();
        let bob_connection: Box<dyn RpcConnection> =
            connections_rx.next().await.unwrap();

        let bob_event = Event::SdpAnswerMade {
            peer_id: PeerId(1),
            sdp_answer: String::from("sdp_answer"),
        };
        bob_connection.send_event("bob_room".into(), bob_event.clone());

        let msgs: Vec<_> = client
            .filter_map(|f| async move {
                if let Frame::Text(text) = f.unwrap() {
                    let server_msg: ServerMsg = serde_json::from_str(
                        std::str::from_utf8(&text).unwrap(),
                    )
                    .unwrap();

                    if !matches!(server_msg, ServerMsg::Ping(_)) {
                        Some(server_msg)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
            .await;
        assert_eq!(
            msgs[0],
            ServerMsg::RpcSettings(RpcSettings {
                idle_timeout_ms: 5000,
                ping_interval_ms: 5000,
            })
        );
        assert_eq!(
            msgs[1],
            ServerMsg::Event {
                room_id: "alice_room".into(),
                event: Event::RoomJoined {
                    member_id: "alice".into(),
                }
            }
        );
        assert_eq!(
            msgs[2],
            ServerMsg::Event {
                room_id: "alice_room".into(),
                event: alice_event,
            }
        );
        assert_eq!(
            msgs[3],
            ServerMsg::Event {
                room_id: "bob_room".into(),
                event: Event::RoomJoined {
                    member_id: "bob".into(),
                }
            }
        );
        assert_eq!(
            msgs[4],
            ServerMsg::Event {
                room_id: "bob_room".into(),
                event: bob_event,
            }
        );
        assert_eq!(msgs.len(), 5);
    }

    #[actix_rt::test]
    async fn close_connection_when_no_active_sessions() {
        let mut serv = test_server(|| -> WsSession {
            let mut rpc_server_repo = MockRpcServerRepository::new();
            rpc_server_repo.expect_get().times(2).returning(|_| {
                let mut rpc_server = MockRpcServer::new();

                rpc_server.expect_connection_established().returning(
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

                Some(Box::new(rpc_server))
            });

            WsSession::new(
                Box::new(rpc_server_repo),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        })
        .await;

        let mut client = serv.ws().await.unwrap();

        client
            .send(into_message(ClientMsg::Command {
                room_id: "room1".into(),
                command: Command::JoinRoom {
                    member_id: "member1".into(),
                    credential: "token".into(),
                },
            }))
            .await
            .unwrap();
        client
            .send(into_message(ClientMsg::Command {
                room_id: "room2".into(),
                command: Command::JoinRoom {
                    member_id: "member2".into(),
                    credential: "token".into(),
                },
            }))
            .await
            .unwrap();

        client
            .send(into_message(ClientMsg::Command {
                room_id: "room1".into(),
                command: Command::LeaveRoom {
                    member_id: "member1".into(),
                },
            }))
            .await
            .unwrap();
        client
            .send(into_message(ClientMsg::Command {
                room_id: "room2".into(),
                command: Command::LeaveRoom {
                    member_id: "member2".into(),
                },
            }))
            .await
            .unwrap();

        let mut frames: Vec<_> =
            client.map(|frame| frame.unwrap()).collect().await;
        assert!(frames.contains(&into_frame(ServerMsg::Event {
            room_id: "room1".into(),
            event: Event::RoomJoined {
                member_id: "member1".into(),
            }
        })));
        assert!(frames.contains(&into_frame(ServerMsg::Event {
            room_id: "room2".into(),
            event: Event::RoomJoined {
                member_id: "member2".into(),
            }
        })));
        assert_eq!(
            frames.pop().unwrap(),
            Frame::Close(Some(CloseReason {
                code: CloseCode::Normal,
                description: Some(String::from("{\"reason\":\"Finished\"}"))
            }))
        )
    }
}
