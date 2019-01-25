use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web::ws::CloseCode;
use actix_web::{
    http, middleware, server, ws, App, AsyncResponder, Error, FutureResponse,
    HttpRequest, HttpResponse, Path,
};
use failure::Fail;
use futures::future::Future;
use hashbrown::{hash_map::Entry, HashMap};

use crate::{
    api::client::*,
    api::control::member::{
        ControlError, GetMember, Id, Member, MemberRepository,
    },
    log::prelude::*,
};

#[derive(Fail, Debug, PartialEq)]
pub enum ClientError {
    #[fail(display = "Session already exists")]
    AlreadyExists,
}

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// websocket connection is long running connection, it easier
/// to handle with an actor
#[derive(Debug)]
pub struct WsSessions {
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    member_id: Id,
}

impl Actor for WsSessions {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. We start the heartbeat process here.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        let join_msg = JoinMember(self.member_id, ctx.address());
        WsSessionRepository::from_registry()
            .send(join_msg)
            .into_actor(self)
            .then(|res, act, ctx| match res {
                Ok(res) => {
                    info!("{:?}", res);
                    if let Err(_) = res {
                        ctx.close(Some(
                            (
                                CloseCode::Normal,
                                "Member already connected!".to_owned(),
                            )
                                .into(),
                        ));
                        ctx.stop();
                    }
                    fut::ok(())
                }
                _ => fut::ok(()),
            })
            .spawn(ctx);
    }
}

/// Handler for `ws::Message`
impl StreamHandler<ws::Message, ws::ProtocolError> for WsSessions {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        // process websocket messages
        println!("WS: {:?}", msg);
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => ctx.text(text),
            ws::Message::Binary(bin) => ctx.binary(bin),
            ws::Message::Close(_) => {
                ctx.stop();
            }
        }
    }
}

impl WsSessions {
    pub fn new(member_id: Id) -> Self {
        Self {
            hb: Instant::now(),
            member_id,
        }
    }

    /// helper method that sends ping to client every second.
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping("");
        });
    }
}

#[derive(Clone)]
pub struct JoinMember(pub Id, pub Addr<WsSessions>);

impl Message for JoinMember {
    type Result = Result<(), ClientError>;
}

#[derive(Clone, Message)]
pub struct LeaveMember(pub Id);

#[derive(Clone, Message)]
#[rtype(result = "bool")]
pub struct IsConnected(pub Id);

impl Handler<IsConnected> for WsSessionRepository {
    type Result = MessageResult<IsConnected>;

    fn handle(
        &mut self,
        msg: IsConnected,
        _: &mut Self::Context,
    ) -> Self::Result {
        debug!("IsConnected message received");
        MessageResult(self.sessions.contains_key(&msg.0))
    }
}

type Client = Addr<WsSessions>;

#[derive(Default)]
pub struct WsSessionRepository {
    sessions: HashMap<Id, Client>,
}

impl Actor for WsSessionRepository {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // self.subscribe_async::<LeaveRoom>(ctx);
        // self.subscribe_async::<SendMessage>(ctx);
    }
}

impl Handler<JoinMember> for WsSessionRepository {
    type Result = Result<(), ClientError>;

    fn handle(
        &mut self,
        msg: JoinMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let JoinMember(id, client) = msg;
        match self.sessions.get(&id) {
            Some(_) => {
                info!("{:?}", self.sessions);
                Err(ClientError::AlreadyExists)
            }
            None => {
                self.sessions.insert(id, client);
                info!("{:?}", self.sessions);
                Ok(())
            }
        }
    }
}

impl Handler<LeaveMember> for WsSessionRepository {
    type Result = ();

    fn handle(&mut self, msg: LeaveMember, _ctx: &mut Self::Context) {
        self.sessions.remove(&msg.0);
        info!("{:?}", self.sessions);
    }
}

impl SystemService for WsSessionRepository {}
impl Supervised for WsSessionRepository {}
