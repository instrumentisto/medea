//! HTTP server for handling WebSocket connections of Client API.

use actix_web::{
    middleware,
    web::{Data, Path, Payload},
    App, Error, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
use futures::{future, Future};
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
// TODO: maybe not use Box<dyn Future...>?
fn ws_index(
    r: HttpRequest,
    info: Path<RequestParams>,
    state: Data<Context>,
    payload: Payload,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    debug!("Request params: {:?}", info);

    match state.rooms.get(info.room_id) {
        Some(room) => Box::new(
            room.send(Authorize {
                member_id: info.member_id,
                credentials: info.credentials.clone(),
            })
            .from_err()
            .and_then(move |res| match res {
                Ok(_) => ws::start(
                    WsSession::new(
                        info.member_id,
                        room,
                        state.config.idle_timeout,
                    ),
                    &r, // TODO: drop_state()
                    payload,
                ),
                Err(AuthorizationError::MemberNotExists) => {
                    Ok(HttpResponse::NotFound().into())
                }
                Err(AuthorizationError::InvalidCredentials) => {
                    Ok(HttpResponse::Forbidden().into())
                }
            }),
        ),
        None => Box::new(future::ok(HttpResponse::NotFound().into())),
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
    //.service(web::resource("/path1").to(|| HttpResponse::Ok()))
    HttpServer::new(move || {
        App::new()
            .data(Context {
                rooms: rooms.clone(),
                config: config.rpc.clone(),
            })
            .service(
                actix_web::web::resource(
                    "/ws/{room_id}/{member_id}/{credentials}",
                )
                .route(actix_web::web::get().to_async(ws_index)),
            )
            .wrap(middleware::Logger::default())
    })
    .bind(server_addr)
    .unwrap()
    .start();

    info!("Started HTTP server on 0.0.0.0:8080");
}

#[cfg(test)]
mod test {
    use std::{ops::Add, thread, time::Duration};

    use actix::{Actor as _, Arbiter, System};
    use actix_codec::{AsyncRead, Framed};
    use actix_http::HttpService;
    use actix_http_test::{TestServer, TestServerRuntime};
    use actix_web::{http, test, App};
    use futures::{sink::Sink, Stream};

    use crate::{
        api::control::Member,
        conf::{Conf, Server, Turn},
        media::create_peers,
        signalling::Room,
        turn::new_turn_auth_service_mock,
    };

    use super::*;
    use actix_http::{
        h1::Message::Item,
        ws::{Frame, Message},
    };
    use futures::future::IntoFuture;
    use tokio::prelude::AsyncWrite;

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
        let arbiter = Arbiter::new();
        let room = Room::start_in_arbiter(&arbiter, move |_| {
            Room::new(
                1,
                members,
                create_peers(1, 2),
                conf.reconnect_timeout,
                new_turn_auth_service_mock(),
            )
        });
        let rooms = hashmap! {1 => room};
        RoomsRepository::new(rooms)
    }

    /// Creates test WebSocket server of Client API which can handle requests.
    fn ws_server(conf: Conf) -> TestServerRuntime {
        TestServer::new(move || {
            HttpService::new(
                App::new()
                    .data(Context {
                        rooms: room(conf.rpc.clone()),
                        config: conf.rpc.clone(),
                    })
                    .service(
                        actix_web::web::resource(
                            "/ws/{room_id}/{member_id}/{credentials}",
                        )
                        .to_async(ws_index),
                    ),
            )
        })
    }

    #[test]
    fn ping_pong_and_disconnects_on_idle() {
        let conf = Conf {
            rpc: Rpc {
                idle_timeout: Duration::new(2, 0),
                reconnect_timeout: Default::default(),
            },
            turn: Turn::default(),
            server: Server::default(),
        };

        let mut server = ws_server(conf.clone());
        let mut socket = server.ws_at("/ws/1/1/caller_credentials").unwrap();

        socket
            .force_send(Message::Text(r#"{"ping": 33}"#.into()))
            .unwrap();

        server
            .block_on(socket.flush().into_future().map_err(|_| ()).and_then(
                |socket| {
                    socket.into_future().map_err(|_| ()).and_then(
                        move |(item, read)| {
                            assert_eq!(
                                Some(ws::Frame::Text(Some(
                                    r#"{"pong":33}"#.into()
                                ))),
                                item
                            );

                            thread::sleep(
                                conf.rpc
                                    .idle_timeout
                                    .add(Duration::from_secs(1)),
                            );

                            read.into_future()
                                .map_err(|(e, _)| panic!("{:?}", e))
                                .map(|(item, _)| {
                                    assert_eq!(
                                        Some(ws::Frame::Close(Some(
                                            ws::CloseCode::Normal.into()
                                        ))),
                                        item
                                    );
                                })
                        },
                    )
                },
            ))
            .unwrap();
    }
}
