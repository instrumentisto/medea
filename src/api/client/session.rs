//! WebSocket session.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use actix::prelude::*;
use actix_web::ws::{self, CloseReason};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

use crate::{
    api::control::member::{self, MemberRepository},
    log::prelude::*,
};

/// Timeout of receiving any WebSocket messages from client.
const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(10); // TODO: via conf

/// Context for [`WsSession`] which holds all the necessary dependencies.
pub struct WsSessionContext {
    /// Repository of all currently existing [`Member`]s in application.
    pub members: MemberRepository,
    /// Repository of all currently existing [`WsSession`]s in application.
    pub sessions: WsSessionRepository,
}

/// Long-running WebSocket connection of Client API.
#[derive(Debug)]
pub struct WsSession {
    /// ID of [`Member`] that WebSocket connection is associated with.
    member_id: member::Id,

    /// Handle for watchdog which checks whether WebSocket client became
    /// idle (no `ping` messages received during [`CLIENT_IDLE_TIMEOUT`]).
    ///
    /// This one should be renewed on any received WebSocket message
    /// from client.
    idle_handler: Option<SpawnHandle>,
}

impl WsSession {
    /// Creates new WebSocket session for specified [`Member`].
    pub fn new(id: member::Id) -> Self {
        Self {
            member_id: id,
            idle_handler: None,
        }
    }

    /// Resets idle handler watchdog.
    fn reset_idle_timeout(&mut self, ctx: &mut <Self as Actor>::Context) {
        if let Some(handler) = self.idle_handler {
            ctx.cancel_future(handler);
        }
        self.idle_handler =
            Some(ctx.run_later(CLIENT_IDLE_TIMEOUT, |_, ctx| {
                debug!("Client timeouted");
                ctx.notify(Close(Some(ws::CloseCode::Away.into())));
            }));
    }
}

/// [`Actor`] implementation that provides an ergonomic way to deal with
/// WebSocket connection lifecycle for [`WsSession`].
impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self, WsSessionContext>;

    /// Starts [`Heartbeat`] mechanism and stores [`WsSession`]
    /// in [`WsSessionRepository`] of application.
    ///
    /// If some [`WsSession`] already exists in [`WsSessionRepository`] for
    /// associated [`Member`], then it will be replaced and closed.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.reset_idle_timeout(ctx);

        let mut repo = ctx.state().sessions.clone();
        if let Some(old) = repo.replace_session(self.member_id, ctx.address()) {
            old.do_send(Close(None));
        }
    }

    /// Removes [`WsSession`] from [`WsSessionRepository`] of application.
    fn stopped(&mut self, ctx: &mut Self::Context) {
        let mut repo = ctx.state().sessions.clone();
        repo.remove_session(self.member_id);
    }
}

/// Message for closing obsolete [`WsSession`] on client reconnection.
#[derive(Message)]
struct Close(Option<CloseReason>);

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
            ws::Message::Close(reason) => ctx.notify(Close(reason)),
            _ => (),
        }
    }
}

/// Repository that stores [`WsSession`]s of [`Member`]s.
#[derive(Clone, Default, Debug)]
pub struct WsSessionRepository {
    sessions: Arc<Mutex<HashMap<member::Id, Addr<WsSession>>>>,
}

impl WsSessionRepository {
    /// Stores [`WsSession`]'s address in repository for given [`Member`]
    /// and returns previous one if any.
    pub fn replace_session(
        &mut self,
        id: member::Id,
        session: Addr<WsSession>,
    ) -> Option<Addr<WsSession>> {
        debug!("add session for member: {}", id);
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(id, session)
    }

    /// Removes address of [`WsSession`] from repository.
    pub fn remove_session(&mut self, id: member::Id) {
        debug!("remove session for member: {}", id);
        let mut sessions = self.sessions.lock().unwrap();
        sessions.remove(&id);
    }
}

#[cfg(test)]
mod test {
    use std::{ops::Add, thread};

    use actix_web::{error, http, test, App};
    use futures::Stream;

    use super::*;

    #[test]
    fn responses_with_pong() {
        let mut srv = test::TestServer::with_factory(move || {
            App::with_state(WsSessionContext {
                members: MemberRepository::default(),
                sessions: WsSessionRepository::default(),
            })
            .resource("/ws/", |r| {
                r.method(http::Method::GET)
                    .with(|r| ws::start(&r, WsSession::new(1)))
            })
        });
        let (reader, mut writer) = srv.ws_at("/ws/").unwrap();

        writer.text(r#"{"ping":33}"#);
        let (item, _reader) = srv.execute(reader.into_future()).unwrap();
        assert_eq!(item, Some(ws::Message::Text(r#"{"pong":33}"#.to_owned())));
    }

    #[test]
    fn disconnects_on_idle() {
        let mut srv = test::TestServer::with_factory(move || {
            App::with_state(WsSessionContext {
                members: MemberRepository::default(),
                sessions: WsSessionRepository::default(),
            })
            .resource("/ws/", |r| {
                r.method(http::Method::GET)
                    .with(|r| ws::start(&r, WsSession::new(1)))
            })
        });
        let (reader, mut writer) = srv.ws_at("/ws/").unwrap();

        thread::sleep(CLIENT_IDLE_TIMEOUT.add(Duration::from_secs(1)));

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
