//! WebSocket session.

use std::{
    convert::TryInto as _,
    fmt::{Display, Error, Formatter},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use actix::{
    fut::WrapFuture as _, Actor, ActorContext, ActorFuture, Addr, Arbiter,
    AsyncContext, ContextFutureSpawner as _, Handler, MailboxError, Message,
    StreamHandler,
};
use actix_http::ws::{CloseReason as WsCloseReason, Item};
use actix_web_actors::ws::{self, CloseCode};
use bytes::{Buf, BytesMut};
use futures::future::{FutureExt as _, LocalBoxFuture};
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason, Event, MemberId, RoomId,
    RpcSettings, ServerMsg,
};

use crate::{
    api::{
        client::rpc_connection::{ClosedReason, EventMessage, RpcConnection},
        RpcServer,
    },
    log::prelude::*,
};

/// Used to generate [`WsSession`] IDs.
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// [`WsSession`] closed reason.
#[derive(Debug)]
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

    /// ID of [`Member`] that WebSocket connection is associated with.
    member_id: MemberId,

    /// ID of [`RpcServer`] that WebSocket connection is associated with.
    room_id: RoomId,

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

    /// Buffer where continuation WebSocket frames are accumulated.
    fragmentation_buffer: BytesMut,

    /// Last number of [`ServerMsg::Ping`].
    last_ping_num: u32,

    /// Interval to send [`ServerMsg::Ping`]s to a client with.
    ping_interval: Duration,

    /// [`WsSession`] closed reason. Should be set by the moment
    /// `Actor::stopped()` for this [`WsSession`] is called.
    close_reason: Option<InnerCloseReason>,
}

impl WsSession {
    /// Creates new [`WsSession`] for specified [`Member`].
    pub fn new(
        member_id: MemberId,
        room_id: RoomId,
        room: Box<dyn RpcServer>,
        idle_timeout: Duration,
        ping_interval: Duration,
    ) -> Self {
        Self {
            id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            member_id,
            room_id,
            room,
            idle_timeout,
            last_activity: Instant::now(),
            fragmentation_buffer: BytesMut::new(),
            last_ping_num: 0,
            ping_interval,
            close_reason: None,
        }
    }

    /// Handles text WebSocket messages.
    fn handle_text(&mut self, text: &str) {
        self.last_activity = Instant::now();
        match serde_json::from_str::<ClientMsg>(&text) {
            Ok(ClientMsg::Pong(n)) => {
                debug!("{}: Received Pong: {}", self, n);
            }
            Ok(ClientMsg::Command(command)) => {
                debug!("{}: Received Command: {:?}", self, command);
                self.room.send_command(self.member_id.clone(), command);
            }
            Err(err) => error!(
                "{}: Error [{:?}] parsing client message: [{}]",
                self, err, &text,
            ),
        }
    }

    /// Handles WebSocket close frame.
    fn handle_close(
        &mut self,
        reason: Option<WsCloseReason>,
        ctx: &mut ws::WebsocketContext<Self>,
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
    fn handle_continuation(&mut self, frame: Item) {
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
                    Ok(text) => self.handle_text(&text),
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
        reason: Close,
    ) {
        debug!("{}: Closing WsSession", self);
        self.close_reason = Some(InnerCloseReason::ByServer);
        ctx.close(Some(reason.0));
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

                Arbiter::spawn(this.room.connection_closed(
                    this.member_id.clone(),
                    ClosedReason::Lost,
                ));

                this.close_in_place(
                    ctx,
                    Close::with_normal_code(&CloseDescription::new(
                        CloseReason::Idle,
                    )),
                );
            }
        });
    }

    /// Sends [`ServerMsg::Ping`] immediately and starts ping send scheduler
    /// with `ping_interval`.
    fn start_pinger(&mut self, ctx: &mut <Self as Actor>::Context) {
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

    /// Returns [`RpcSettings`] based on `idle_timeout` and `ping_interval`
    /// settled for this [`WsSession`].
    fn get_rpc_settings(&self) -> RpcSettings {
        RpcSettings {
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
        }
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

        self.room
            .connection_established(
                self.member_id.clone(),
                Box::new(ctx.address()),
            )
            .into_actor(self)
            .then(move |result, this, ctx| {
                if result.is_ok() {
                    let rpc_settings_message = serde_json::to_string(
                        &ServerMsg::RpcSettings(this.get_rpc_settings()),
                    )
                        .unwrap();
                    ctx.text(rpc_settings_message);

                    this.start_pinger(ctx);
                    Self::start_idle_watchdog(ctx);
                } else {
                    error!(
                        "{}: WsSession of Member failed to join Room",
                        this,
                    );
                    this.close_in_place(
                        ctx,
                        Close::with_normal_code(&CloseDescription::new(
                            CloseReason::InternalError,
                        )),
                    );
                };
                actix::fut::ready(())
            })
            .wait(ctx);
    }

    /// Invokes `RpcServer::connection_closed()` with `ClosedReason::Lost` if
    /// `WsSession.close_reason` is `None`, with [`ClosedReason`] defined in
    /// `WsSession.close_reason` if it is `Some(InnerCloseReason::ByClient)`,
    /// does nothing if `WsSession.close_reason` is
    /// `Some(InnerCloseReason::ByServer)`.
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("{}: WsSession Stopped", self);
        match self.close_reason.take() {
            None => {
                error!("{}: WsSession was unexpectedly dropped", self);
                Arbiter::spawn(self.room.connection_closed(
                    self.member_id.clone(),
                    ClosedReason::Lost,
                ));
            }
            Some(InnerCloseReason::ByClient(reason)) => Arbiter::spawn(
                self.room.connection_closed(self.member_id.clone(), reason),
            ),
            Some(InnerCloseReason::ByServer) => {
                // do nothing
            }
        }
    }
}

impl RpcConnection for Addr<WsSession> {
    /// Closes [`WsSession`] by sending itself "normal closure" close message
    /// with [`CloseDescription`] as description of [Close] frame.
    ///
    /// [Close]:https://tools.ietf.org/html/rfc6455#section-5.5.1
    fn close(
        &mut self,
        close_description: CloseDescription,
    ) -> LocalBoxFuture<'static, ()> {
        let close_result =
            self.send(Close::with_normal_code(&close_description));
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
    fn send_event(&self, msg: Event) {
        self.do_send(EventMessage::from(msg));
    }
}

/// Message for closing [`WsSession`].
#[derive(Message)]
#[rtype(result = "()")]
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
        self.close_in_place(ctx, close);
    }
}

impl Handler<EventMessage> for WsSession {
    type Result = ();

    /// Sends [`Event`] to Web Client.
    fn handle(&mut self, msg: EventMessage, ctx: &mut Self::Context) {
        debug!("{}: Sending Event: {:?}]", self, msg);
        let event =
            serde_json::to_string(&ServerMsg::Event(msg.into())).unwrap();
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
                ws::Message::Text(text) => self.handle_text(&text),
                ws::Message::Close(reason) => self.handle_close(reason, ctx),
                ws::Message::Continuation(item) => {
                    self.handle_continuation(item);
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
                    Close::with_normal_code(&CloseDescription {
                        reason: CloseReason::InternalError,
                    }),
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
        write!(
            f,
            "WsSession [{}] of Member [{}/{}]",
            self.id, self.room_id, self.member_id
        )
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
        Event, IceCandidate, MemberId, PeerId, RoomId, RpcSettings, ServerMsg,
    };
    use tokio::time::timeout;

    use crate::api::{
        client::rpc_connection::{ClosedReason, RpcConnection},
        MockRpcServer,
    };

    use super::WsSession;

    type SharedOneshot<T> =
        (Mutex<Option<Sender<T>>>, Mutex<Option<Receiver<T>>>);
    type SharedUnbounded<T> = (
        Mutex<UnboundedSender<T>>,
        Mutex<Option<UnboundedReceiver<T>>>,
    );

    fn server_msg_into_frame(msg: &ServerMsg) -> Frame {
        Frame::Text(serde_json::to_string(msg).unwrap().into())
    }

    fn client_msg_into_bytes(msg: &ClientMsg) -> Bytes {
        Bytes::from(serde_json::to_string(msg).unwrap())
    }

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
            let member_id = MemberId::from("test_member");
            let mut rpc_server = MockRpcServer::new();

            let expected_member_id = member_id.clone();
            rpc_server
                .expect_connection_established()
                .withf(move |member_id, _| *member_id == expected_member_id)
                .return_once(|_, _| future::err(()).boxed_local());
            rpc_server
                .expect_connection_closed()
                .returning(|_, _| future::ready(()).boxed_local());

            WsSession::new(
                member_id,
                RoomId::from("room"),
                Box::new(rpc_server),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        }

        let mut serv = test_server(factory);

        let mut client = serv.ws().await.unwrap();

        let item = client.next().await.unwrap().unwrap();

        let close_frame = Frame::Close(Some(CloseReason {
            code: CloseCode::Normal,
            description: Some(String::from(r#"{"reason":"InternalError"}"#)),
        }));

        assert_eq!(item, close_frame);
    }

    #[actix_rt::test]
    async fn sends_rpc_settings_and_pings() {
        let mut serv = test_server(|| -> WsSession {
            let member_id = MemberId::from("test_member");
            let mut rpc_server = MockRpcServer::new();

            rpc_server
                .expect_connection_established()
                .return_once(|_, _| future::ok(()).boxed_local());
            rpc_server
                .expect_connection_closed()
                .returning(|_, _| future::ready(()).boxed_local());

            WsSession::new(
                member_id,
                RoomId::from("room"),
                Box::new(rpc_server),
                Duration::from_secs(5),
                Duration::from_millis(50),
            )
        });

        let mut client = serv.ws().await.unwrap();
        let item = client.next().await.unwrap().unwrap();
        assert_eq!(
            item,
            server_msg_into_frame(&ServerMsg::RpcSettings(RpcSettings {
                idle_timeout_ms: 5000,
                ping_interval_ms: 50,
            }))
        );

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(item, server_msg_into_frame(&ServerMsg::Ping(0)));

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(item, server_msg_into_frame(&ServerMsg::Ping(1)));
    }

    // WsSession is dropped and WebSocket connection is closed if no pongs
    // received for idle_timeout.
    #[actix_rt::test]
    async fn dropped_if_idle() {
        let mut serv = test_server(|| -> WsSession {
            let member_id = MemberId::from("test_member");
            let mut rpc_server = MockRpcServer::new();

            rpc_server
                .expect_connection_established()
                .return_once(|_, _| future::ok(()).boxed_local());

            let expected_member_id = member_id.clone();
            rpc_server
                .expect_connection_closed()
                .withf(move |member_id, reason| {
                    *member_id == expected_member_id
                        && *reason == ClosedReason::Lost
                })
                .return_once(|_, _| future::ready(()).boxed_local());

            WsSession::new(
                member_id,
                RoomId::from("room"),
                Box::new(rpc_server),
                Duration::from_millis(100),
                Duration::from_secs(10),
            )
        });

        let client = serv.ws().await.unwrap();

        let start = std::time::Instant::now();

        let item = client.skip(2).next().await.unwrap().unwrap();

        let close_frame = Frame::Close(Some(CloseReason {
            code: CloseCode::Normal,
            description: Some(String::from(r#"{"reason":"Idle"}"#)),
        }));

        assert!(
            Instant::now().duration_since(start) > Duration::from_millis(99)
        );
        assert!(Instant::now().duration_since(start) < Duration::from_secs(2));
        assert_eq!(item, close_frame);
    }

    // Make sure that WsSession redirects all Commands it receives to RpcServer.
    #[actix_rt::test]
    async fn passes_commands_to_rpc_server() {
        lazy_static::lazy_static! {
            static ref CHAN: SharedUnbounded<Command> = {
                let (tx, rx) = mpsc::unbounded();
                (Mutex::new(tx), Mutex::new(Some(rx)))
            };
        }

        let mut serv = test_server(|| -> WsSession {
            let member_id = MemberId::from("test_member");
            let mut rpc_server = MockRpcServer::new();

            rpc_server
                .expect_connection_established()
                .return_once(|_, _| future::ok(()).boxed_local());
            rpc_server
                .expect_connection_closed()
                .returning(|_, _| future::ready(()).boxed_local());

            rpc_server.expect_send_command().returning(|_, command| {
                CHAN.0.lock().unwrap().unbounded_send(command).unwrap();
            });

            WsSession::new(
                member_id,
                RoomId::from("room"),
                Box::new(rpc_server),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        });

        let mut client = serv.ws().await.unwrap();

        let command = client_msg_into_bytes(&ClientMsg::Command(
            Command::SetIceCandidate {
                peer_id: PeerId(15),
                candidate: IceCandidate {
                    candidate: "asd".to_string(),
                    sdp_m_line_index: Some(1),
                    sdp_mid: Some("2".to_string()),
                },
            },
        ));
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
            let member_id = MemberId::from("test_member");
            let mut rpc_server = MockRpcServer::new();

            rpc_server.expect_connection_established().return_once(
                |_, connection| {
                    let _ =
                        CHAN.0.lock().unwrap().take().unwrap().send(connection);
                    future::ok(()).boxed_local()
                },
            );
            rpc_server
                .expect_connection_closed()
                .returning(|_, _| future::ready(()).boxed_local());

            WsSession::new(
                member_id,
                RoomId::from("room"),
                Box::new(rpc_server),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        });

        let client = serv.ws().await.unwrap();

        let mut rpc_connection: Box<dyn RpcConnection> =
            CHAN.1.lock().unwrap().take().unwrap().await.unwrap();

        rpc_connection
            .close(CloseDescription {
                reason: ProtoCloseReason::Evicted,
            })
            .await;

        let item = client.skip(2).next().await.unwrap().unwrap();

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
            let member_id = MemberId::from("test_member");
            let mut rpc_server = MockRpcServer::new();

            rpc_server.expect_connection_established().return_once(
                |_, connection| {
                    let _ =
                        CHAN.0.lock().unwrap().take().unwrap().send(connection);
                    async { Ok(()) }.boxed_local()
                },
            );
            rpc_server
                .expect_connection_closed()
                .returning(|_, _| future::ready(()).boxed_local());

            WsSession::new(
                member_id,
                RoomId::from("room"),
                Box::new(rpc_server),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        });

        let client = serv.ws().await.unwrap();

        let rpc_connection: Box<dyn RpcConnection> =
            CHAN.1.lock().unwrap().take().unwrap().await.unwrap();

        rpc_connection.send_event(Event::SdpAnswerMade {
            peer_id: PeerId(77),
            sdp_answer: String::from("sdp_answer"),
        });

        let item = client.skip(2).next().await.unwrap().unwrap();
        assert_eq!(
            item,
            server_msg_into_frame(&ServerMsg::Event(Event::SdpAnswerMade {
                peer_id: PeerId(77),
                sdp_answer: "sdp_answer".to_string(),
            }))
        );
    }
}
