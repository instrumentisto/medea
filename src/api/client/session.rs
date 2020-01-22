//! WebSocket session.

use std::{
    convert::TryInto as _,
    time::{Duration, Instant},
};

use actix::{
    fut::WrapFuture as _, Actor, ActorContext, ActorFuture, Addr, Arbiter,
    AsyncContext, ContextFutureSpawner as _, Handler, Message, StreamHandler,
};
use actix_web_actors::ws::{self, CloseCode};
use futures::future::{FutureExt as _, LocalBoxFuture};
use medea_client_api_proto::{
    ClientMsg, CloseDescription, CloseReason, Event, RpcSettings, ServerMsg,
};

use crate::{
    api::{
        client::rpc_connection::{ClosedReason, EventMessage, RpcConnection},
        control::MemberId,
        RpcServer,
    },
    log::prelude::*,
};

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

    /// Last number of [`ServerMsg::Ping`].
    last_ping_num: u64,

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
        room: Box<dyn RpcServer>,
        idle_timeout: Duration,
        ping_interval: Duration,
    ) -> Self {
        Self {
            member_id,
            room,
            idle_timeout,
            last_activity: Instant::now(),
            last_ping_num: 0,
            ping_interval,
            close_reason: None,
        }
    }

    /// Starts watchdog which will drop connection if `now`-`last_activity` >
    /// `idle_timeout`.
    fn start_idle_watchdog(ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(Duration::new(1, 0), |session, ctx| {
            if Instant::now().duration_since(session.last_activity)
                > session.idle_timeout
            {
                info!("WsSession of member {} is idle", session.member_id);

                Arbiter::spawn(session.room.connection_closed(
                    session.member_id.clone(),
                    ClosedReason::Lost,
                ));

                ctx.notify(Close::with_normal_code(&CloseDescription::new(
                    CloseReason::Idle,
                )))
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
        debug!("Started WsSession for Member [id = {}]", self.member_id);

        self.room
            .connection_established(
                self.member_id.clone(),
                Box::new(ctx.address()),
            )
            .into_actor(self)
            .then(move |result, this, ctx| {
                match result {
                    Ok(_) => {
                        // send RpcSettings
                        let rpc_settings_message =
                            serde_json::to_string(&this.get_rpc_settings())
                                .unwrap();
                        ctx.text(rpc_settings_message);

                        this.start_pinger(ctx);
                        Self::start_idle_watchdog(ctx);
                    }
                    Err(err) => {
                        error!(
                            "WsSession of member {} failed to join Room, \
                             because: {:?}",
                            this.member_id, err,
                        );
                        ctx.notify(Close::with_normal_code(
                            &CloseDescription::new(CloseReason::InternalError),
                        ));
                    }
                };
                async {}.into_actor(this)
            })
            .wait(ctx);
    }

    /// Invokes `RpcServer::connection_closed()` with `ClosedReason::Lost` if
    /// `WsSession.close_reason` is `None`, with [`ClosedReason`] defined in
    /// `WsSession.close_reason` if it is `Some(InnerCloseReason::ByClient)`,
    /// does nothing if `WsSession.close_reason` is
    /// `Some(InnerCloseReason::ByServer)`.
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("Stopped WsSession for member {}", self.member_id);
        match self.close_reason.take() {
            None => {
                error!(
                    "WsSession of Member [{}] was unexpectedly dropped",
                    self.member_id
                );
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
                error!("Failed Close RpcConnection: {:?} ", err)
            }
        }
        .boxed_local()
    }

    /// Sends [`Event`] to Web Client.
    ///
    /// [`Event`]: medea_client_api_proto::Event
    fn send_event(
        &self,
        msg: Event,
    ) -> LocalBoxFuture<'static, Result<(), ()>> {
        let send_result = self.send(EventMessage::from(msg));
        async {
            match send_result.await {
                Ok(_) => Ok(()),
                Err(err) => {
                    error!("Failed send Event to RpcConnection: {:?} ", err);
                    Err(())
                }
            }
        }
        .boxed_local()
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
        debug!("Closing WsSession for member {}", self.member_id);
        self.close_reason = Some(InnerCloseReason::ByServer);
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

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    /// Handles arbitrary [`ws::Message`] received from WebSocket client.
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        debug!(
            "Received WS message: {:?} from member {}",
            msg, self.member_id
        );
        match msg {
            Ok(msg) => match msg {
                ws::Message::Text(text) => {
                    self.last_activity = Instant::now();
                    match serde_json::from_str::<ClientMsg>(&text) {
                        Ok(ClientMsg::Pong(_)) => {
                            // do nothing
                        }
                        Ok(ClientMsg::Command(command)) => {
                            self.room
                                .send_command(command)
                                .into_actor(self)
                                .spawn(ctx);
                        }
                        Err(err) => error!(
                            "Error [{:?}] parsing client message [{}]",
                            err, &text
                        ),
                    }
                }
                ws::Message::Close(reason) => {
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

                        self.close_reason =
                            Some(InnerCloseReason::ByClient(closed_reason));
                        ctx.close(reason);
                        ctx.stop();
                    }
                }
                _ => error!(
                    "Unsupported client message from member {}",
                    self.member_id
                ),
            },
            Err(err) => {
                error!(
                    "Error in WsSession StreamHandler for Member [{}]: {:?}",
                    self.member_id, err
                );
            }
        };
    }

    /// Method is called when stream finishes. Stops [`WsSession`] actor
    /// execution.
    fn finished(&mut self, ctx: &mut Self::Context) {
        debug!(
            "WsSession message stream for Member [{}] is finished",
            self.member_id
        );
        ctx.stop()
    }
}

#[cfg(test)]
mod test {

    use std::{
        sync::Mutex,
        time::{Duration, Instant},
    };

    use actix_web::{test::TestServer, web, App, HttpRequest};
    use actix_web_actors::ws::{start, CloseCode, CloseReason, Frame, Message};
    use medea_client_api_proto::{
        CloseDescription, CloseReason as ProtoCloseReason, Command, Event,
        PeerId,
    };

    use futures::{
        channel::oneshot::{self, Receiver, Sender},
        FutureExt as _, SinkExt as _, StreamExt as _,
    };

    use crate::api::{
        client::rpc_connection::{ClosedReason, RpcConnection},
        control::MemberId,
        MockRpcServer,
    };

    use super::WsSession;

    type SharedChan<T> = (Mutex<Option<Sender<T>>>, Mutex<Option<Receiver<T>>>);

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
            let member_id = MemberId::from(String::from("test_member"));
            let mut rpc_server = MockRpcServer::new();

            let expected_member_id = member_id.clone();
            rpc_server
                .expect_connection_established()
                .withf(move |member_id, _| *member_id == expected_member_id)
                .return_once(|_, _| async { Err(()) }.boxed_local());
            rpc_server
                .expect_connection_closed()
                .returning(|_, _| async {}.boxed_local());

            WsSession::new(
                member_id,
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
            let member_id = MemberId::from(String::from("test_member"));
            let mut rpc_server = MockRpcServer::new();

            rpc_server
                .expect_connection_established()
                .return_once(|_, _| async { Ok(()) }.boxed_local());
            rpc_server
                .expect_connection_closed()
                .returning(|_, _| async {}.boxed_local());

            WsSession::new(
                member_id,
                Box::new(rpc_server),
                Duration::from_secs(5),
                Duration::from_millis(50),
            )
        });

        let mut client = serv.ws().await.unwrap();
        let item = client.next().await.unwrap().unwrap();
        assert_eq!(
            item,
            Frame::Text(
                String::from(
                    r#"{"idle_timeout_ms":5000,"ping_interval_ms":50}"#
                )
                .into()
            )
        );

        let item = client.next().await.unwrap().unwrap();
        assert_eq!(item, Frame::Text(String::from(r#"{"ping":0}"#).into()));

        let item = client.next().await.unwrap().unwrap();

        assert_eq!(item, Frame::Text(String::from(r#"{"ping":1}"#).into()));
    }

    // WsSession is dropped and WebSocket connection is closed if no pongs
    // received for idle_timeout.
    #[actix_rt::test]
    async fn dropped_if_idle() {
        let mut serv = test_server(|| -> WsSession {
            let member_id = MemberId::from(String::from("test_member"));
            let mut rpc_server = MockRpcServer::new();

            rpc_server
                .expect_connection_established()
                .return_once(|_, _| async { Ok(()) }.boxed_local());

            let expected_member_id = member_id.clone();
            rpc_server
                .expect_connection_closed()
                .withf(move |member_id, reason| {
                    *member_id == expected_member_id
                        && *reason == ClosedReason::Lost
                })
                .return_once(|_, _| async {}.boxed_local());

            WsSession::new(
                member_id,
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
            static ref CHAN: SharedChan<Command> = {
                let (tx, rx) = oneshot::channel();
                (Mutex::new(Some(tx)), Mutex::new(Some(rx)))
            };
        }

        let mut serv = test_server(|| -> WsSession {
            let member_id = MemberId::from(String::from("test_member"));
            let mut rpc_server = MockRpcServer::new();

            rpc_server
                .expect_connection_established()
                .return_once(|_, _| async { Ok(()) }.boxed_local());
            rpc_server
                .expect_connection_closed()
                .returning(|_, _| async {}.boxed_local());

            rpc_server.expect_send_command().return_once(|command| {
                let _ = CHAN.0.lock().unwrap().take().unwrap().send(command);
                async {}.boxed_local()
            });

            WsSession::new(
                member_id,
                Box::new(rpc_server),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        });

        let mut client = serv.ws().await.unwrap();

        let command = r#"{
                            "command":"SetIceCandidate",
                                "data":{
                                    "peer_id":15,
                                    "candidate":{
                                        "candidate":"asd",
                                        "sdp_m_line_index":1,
                                        "sdp_mid":"2"
                                    }
                                }
                            }"#;

        client
            .send(Message::Text(String::from(command)))
            .await
            .unwrap();

        let command = CHAN.1.lock().unwrap().take().unwrap().await.unwrap();
        match command {
            Command::SetIceCandidate { peer_id, candidate } => {
                assert_eq!(peer_id.0, 15);
                assert_eq!(candidate.candidate, "asd");
            }
            _ => unreachable!(),
        }
    }

    // WsSession is dropped and WebSocket connection is closed when
    // RpcConnection::close is called.
    #[actix_rt::test]
    async fn close_when_rpc_connection_close() {
        lazy_static::lazy_static! {
            static ref CHAN: SharedChan<Box<dyn RpcConnection>> = {
                let (tx, rx) = oneshot::channel();
                (Mutex::new(Some(tx)), Mutex::new(Some(rx)))
            };
        }

        let mut serv = test_server(|| -> WsSession {
            let member_id = MemberId::from(String::from("test_member"));
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
                .returning(|_, _| async {}.boxed_local());

            WsSession::new(
                member_id,
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
            static ref CHAN: SharedChan<Box<dyn RpcConnection>> = {
                let (tx, rx) = oneshot::channel();
                (Mutex::new(Some(tx)), Mutex::new(Some(rx)))
            };
        }

        let mut serv = test_server(|| -> WsSession {
            let member_id = MemberId::from(String::from("test_member"));
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
                .returning(|_, _| async {}.boxed_local());

            WsSession::new(
                member_id,
                Box::new(rpc_server),
                Duration::from_secs(5),
                Duration::from_secs(5),
            )
        });

        let client = serv.ws().await.unwrap();

        let rpc_connection: Box<dyn RpcConnection> =
            CHAN.1.lock().unwrap().take().unwrap().await.unwrap();

        rpc_connection
            .send_event(Event::SdpAnswerMade {
                peer_id: PeerId(77),
                sdp_answer: String::from("sdp_answer"),
            })
            .await
            .unwrap();

        let item = client.skip(2).next().await.unwrap().unwrap();

        let event = "{\"event\":\"SdpAnswerMade\",\"data\":{\"peer_id\":77,\"\
                     sdp_answer\":\"sdp_answer\"}}";

        assert_eq!(item, Frame::Text(event.into()));
    }
}
