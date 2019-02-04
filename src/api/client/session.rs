//! Member websocket session definitions and implementations.

use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web::ws;
use hashbrown::HashMap;

use crate::{api::client::AppState, api::control::member::Id, log::prelude::*};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Websocket connection is long running connection, it easier
/// to handle with an actor.
#[derive(Debug)]
pub struct WsSessions {
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    member_id: Id,
}

impl WsSessions {
    /// Creates new [`Member`] session with passed-in [`Member`] ID.
    pub fn new(member_id: Id) -> Self {
        Self {
            hb: Instant::now(),
            member_id,
        }
    }

    /// Helper method that sends ping to client every second.
    ///
    /// Also this method checks heartbeats from client
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                ctx.stop();
                return;
            }
            ctx.ping("");
        });
    }
}

impl Actor for WsSessions {
    type Context = ws::WebsocketContext<Self, AppState>;

    /// Start the heartbeat process and store session in repository
    /// on start [`Member`] session.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        let mut session_repo = ctx.state().session_repo.lock().unwrap();
        session_repo.add_session(self.member_id, ctx.address());
    }

    /// Remove [`Member`] session repository after stopped session.
    fn stopped(&mut self, ctx: &mut Self::Context) {
        let mut session_repo = ctx.state().session_repo.lock().unwrap();
        session_repo.remove_session(self.member_id);
    }
}

/// Handler for `ws::Message`
impl StreamHandler<ws::Message, ws::ProtocolError> for WsSessions {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
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

/// Address of [`Member`] session for communicate with it.
type Client = Addr<WsSessions>;

/// Repository that stores [`Member`] sessions.
#[derive(Default, Debug)]
pub struct WsSessionRepository {
    sessions: HashMap<Id, Client>,
}

impl WsSessionRepository {
    /// Stores address of [`Member`] session in repository.
    pub fn add_session(&mut self, id: Id, client: Client) {
        debug!("add session for member: {}", id);
        self.sessions.insert(id, client);
    }

    /// Removes address of [`Member`] session in repository.
    pub fn remove_session(&mut self, id: Id) {
        debug!("remove session for member: {}", id);
        self.sessions.remove(&id);
    }
}
