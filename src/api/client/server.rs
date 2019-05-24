//! HTTP server for handling WebSocket connections of Client API.

use actix_web::{
    http, middleware, server, ws, App, AsyncResponder, FutureResponse,
    HttpRequest, HttpResponse, Path, State,
};
use futures::{future, Future as _};
use serde::Deserialize;

use crate::{
    api::{
        client::{
            rpc_connection::{AuthorizationError, Authorize},
            session::WsSession,
        },
        control::MemberId,
    },
    conf::{Conf, Rpc},
    log::prelude::*,
    signalling::{RoomId, RoomsRepository},
};

/// Parameters of new WebSocket connection creation HTTP request.
#[derive(Debug, Deserialize)]
struct RequestParams {
    /// ID of [`Room`] that WebSocket connection connects to.
    room_id: RoomId,
    /// ID of [`Member`] that establishes WebSocket connection.
    member_id: MemberId,
    /// Credential of [`Member`] to authorize WebSocket connection with.
    credentials: String,
}

/// Handles all HTTP requests, performs WebSocket handshake (upgrade) and starts
/// new [`WsSession`] for WebSocket connection.
fn ws_index(
    (r, info, state): (
        HttpRequest<Context>,
        Path<RequestParams>,
        State<Context>,
    ),
) -> FutureResponse<HttpResponse> {
    debug!("Request params: {:?}", info);

    match state.rooms.get(info.room_id) {
        Some(room) => room
            .send(Authorize {
                member_id: info.member_id,
                credentials: info.credentials.clone(),
            })
            .from_err()
            .and_then(move |res| match res {
                Ok(_) => ws::start(
                    &r.drop_state(),
                    WsSession::new(
                        info.member_id,
                        room,
                        state.config.idle_timeout,
                    ),
                ),
                Err(AuthorizationError::MemberNotExists) => {
                    Ok(HttpResponse::NotFound().into())
                }
                Err(AuthorizationError::InvalidCredentials) => {
                    Ok(HttpResponse::Forbidden().into())
                }
            })
            .responder(),
        None => future::ok(HttpResponse::NotFound().into()).responder(),
    }
}

/// Context for [`App`] which holds all the necessary dependencies.
pub struct Context {
    /// Repository of all currently existing [`Room`]s in application.
    pub rooms: RoomsRepository,

    /// Settings of application.
    pub config: Rpc,
}

/// Starts HTTP server for handling WebSocket connections of Client API.
pub fn run(rooms: RoomsRepository, config: Conf) {
    let server_addr = config.server.bind_addr();

    server::new(move || {
        App::with_state(Context {
            rooms: rooms.clone(),
            config: config.rpc.clone(),
        })
        .middleware(middleware::Logger::default())
        .resource("/ws/{room_id}/{member_id}/{credentials}", |r| {
            r.method(http::Method::GET).with(ws_index)
        })
    })
    .bind(server_addr)
    .unwrap()
    .start();

    info!("Started HTTP server on 0.0.0.0:8080");
}

#[cfg(test)]
mod test {
    use std::{ops::Add, thread, time::Duration};

    use actix::Arbiter;
    use actix_web::{http, test, App};
    use futures::Stream;

    use crate::{
        api::control::Member,
        conf::{Conf, Server, Turn},
        media::create_peers,
        signalling::Room,
    };

    use super::*;

    /// Creates [`RoomsRepository`] for tests filled with a single [`Room`].
    fn room(conf: Rpc) -> RoomsRepository {
        let members = hashmap! {
            1 => Member{
                id: 1,
                credentials: "caller_credentials".into(),
                ice_user: None
            },
            2 => Member{
                id: 2,
                credentials: "responder_credentials".into(),
                ice_user: None
            },
        };
        let room = Arbiter::start(move |_| {
            Room::new(
                1,
                members,
                create_peers(1, 2),
                conf.reconnect_timeout,
                crate::turn::dummy(),
            )
        });
        let rooms = hashmap! {1 => room};
        RoomsRepository::new(rooms)
    }

    /// Creates test WebSocket server of Client API which can handle requests.
    fn ws_server(conf: Conf) -> test::TestServer {
        test::TestServer::with_factory(move || {
            App::with_state(Context {
                rooms: room(conf.rpc.clone()),
                config: conf.rpc.clone(),
            })
            .resource("/ws/{room_id}/{member_id}/{credentials}", |r| {
                r.method(http::Method::GET).with(ws_index)
            })
        })
    }

    #[test]
    fn responses_with_pong() {
        let mut server = ws_server(Conf::default());
        let (read, mut write) =
            server.ws_at("/ws/1/1/caller_credentials").unwrap();

        write.text(r#"{"ping":33}"#);
        let (item, _) = server.execute(read.into_future()).unwrap();
        assert_eq!(Some(ws::Message::Text(r#"{"pong":33}"#.into())), item);
    }

    #[test]
    fn disconnects_on_idle() {
        let conf = Conf {
            rpc: Rpc {
                idle_timeout: Duration::new(2, 0),
                reconnect_timeout: Default::default(),
            },
            turn: Turn::default(),
            server: Server::default(),
        };

        let mut server = ws_server(conf.clone());
        let (read, mut write) =
            server.ws_at("/ws/1/1/caller_credentials").unwrap();

        write.text(r#"{"ping":33}"#);
        let (item, read) = server.execute(read.into_future()).unwrap();
        assert_eq!(Some(ws::Message::Text(r#"{"pong":33}"#.into())), item);

        thread::sleep(conf.rpc.idle_timeout.add(Duration::from_secs(1)));

        let (item, _) = server.execute(read.into_future()).unwrap();
        assert_eq!(
            Some(ws::Message::Close(Some(ws::CloseCode::Normal.into()))),
            item
        );
    }
}
