//! Member websocket session definitions and implementations.

use std::time::Duration;

use actix::prelude::*;
use actix_web::ws;
use actix_web::ws::CloseReason;
use hashbrown::HashMap;

use crate::{
    api::client::{AppState, Command, Event},
    api::control::member::Id,
    log::prelude::*,
};

/// How long before lack of client message causes a timeout.
const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Message for close old member session when reconnect [`Web Client`].
#[derive(Message)]
struct Close(Option<CloseReason>);

/// Websocket connection is long running connection, it easier
/// to handle with an actor.
#[derive(Debug)]
pub struct WsSessions {
    member_id: Id,
    /// Client must send any text message at least once per 10 seconds
    /// (CLIENT_IDLE_TIMEOUT), otherwise we drop connection.
    idle_timeout_handler: Option<SpawnHandle>,
}

impl WsSessions {
    /// Creates new [`Member`] session with passed-in [`Member`] ID.
    pub fn new(member_id: Id) -> Self {
        Self {
            member_id,
            idle_timeout_handler: None,
        }
    }

    /// Helper method that update read timeout handler after every message
    /// from [`Web Client`].
    fn hb(&mut self, ctx: &mut <Self as Actor>::Context) {
        if let Some(handler) = self.idle_timeout_handler {
            ctx.cancel_future(handler);
        }
        self.idle_timeout_handler =
            Some(ctx.run_later(CLIENT_IDLE_TIMEOUT, |_, ctx| {
                debug!("Client timeout");
                ctx.close(Some(ws::CloseCode::Away.into()));
                ctx.stop();
            }));
    }
}

impl Actor for WsSessions {
    type Context = ws::WebsocketContext<Self, AppState>;

    /// Start the heartbeat process and store session in repository
    /// on start [`Member`] session.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        let mut session_repo = ctx.state().session_repo.lock().unwrap();
        if let Some(old) =
            session_repo.add_session(self.member_id, ctx.address())
        {
            old.do_send(Close(None));
        }
    }

    /// Remove [`Member`] session repository after stopped session.
    fn stopped(&mut self, ctx: &mut Self::Context) {
        let mut session_repo = ctx.state().session_repo.lock().unwrap();
        session_repo.remove_session(self.member_id);
    }
}

/// Handler for `Close`.
impl Handler<Close> for WsSessions {
    type Result = ();

    fn handle(&mut self, close: Close, ctx: &mut Self::Context) {
        ctx.close(close.0);
        ctx.stop();
    }
}

/// Handler for `Command`.
impl Handler<Command> for WsSessions {
    type Result = ();

    fn handle(&mut self, command: Command, ctx: &mut Self::Context) {
        match command {
            Command::Ping(n) => {
                ctx.text(serde_json::to_string(&Event::Pong(n)).unwrap())
            }
        };
    }
}

/// Handler for `ws::Message`
impl StreamHandler<ws::Message, ws::ProtocolError> for WsSessions {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Text(text) => {
                match serde_json::from_str::<Command>(&text) {
                    Ok(command) => {
                        debug!("Received command:\n{:?}\n", command);
                        ctx.notify(command);
                        self.hb(ctx);
                    }
                    Err(e) => {
                        ctx.text(format!("Could not parse command: {}\n", e));
                    }
                }
            }
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
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
    pub fn add_session(&mut self, id: Id, client: Client) -> Option<Client> {
        debug!("add session for member: {}", id);
        self.sessions.insert(id, client)
    }

    /// Removes address of [`Member`] session in repository.
    pub fn remove_session(&mut self, id: Id) {
        debug!("remove session for member: {}", id);
        self.sessions.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::thread;

    use actix_web::{error, http, test, App};
    use futures::stream::Stream;

    use super::*;
    use crate::api::control::*;

    #[test]
    fn connect_by_credentials() {
        let members_repo = Arc::new(Mutex::new(MemberRepository::default()));
        let session_repo = Arc::new(Mutex::new(WsSessionRepository::default()));

        let mut srv = test::TestServer::with_factory(move || {
            App::with_state(AppState {
                members_repo: members_repo.clone(),
                session_repo: session_repo.clone(),
            })
            .resource("/ws/", |r| {
                r.method(http::Method::GET)
                    .with(|r| ws::start(&r, WsSessions::new(1)))
            })
        });
        let (reader, mut writer) = srv.ws_at("/ws/").unwrap();

        writer.text(r#"{"ping":33}"#);
        let (item, _reader) = srv.execute(reader.into_future()).unwrap();
        assert_eq!(item, Some(ws::Message::Text(r#"{"pong":33}"#.to_owned())));
    }

    #[test]
    fn disconnect_by_timeout() {
        let members_repo = Arc::new(Mutex::new(MemberRepository::default()));
        let session_repo = Arc::new(Mutex::new(WsSessionRepository::default()));

        let mut srv = test::TestServer::with_factory(move || {
            App::with_state(AppState {
                members_repo: members_repo.clone(),
                session_repo: session_repo.clone(),
            })
            .resource("/ws/{credentials}", |r| {
                r.method(http::Method::GET)
                    .with(|r| ws::start(&r, WsSessions::new(1)))
            })
        });
        let (reader, mut writer) = srv.ws_at("/ws/caller_credentials").unwrap();

        thread::sleep(CLIENT_IDLE_TIMEOUT);

        writer.text(r#"{"ping":33}"#);
        assert!(match srv.execute(reader.into_future()) {
            Err((
                ws::ProtocolError::Payload(error::PayloadError::Io(_)),
                _,
            )) => true,
            _ => false,
        });
    }
}
