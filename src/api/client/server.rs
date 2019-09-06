//! HTTP server for handling WebSocket connections of Client API.

use std::io;

use actix::{Actor, Addr, Handler, ResponseFuture};
use actix_web::{
    dev::Server as ActixServer,
    middleware,
    web::{resource, Data, Path, Payload},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
use futures::{
    future::{self, Either},
    Future,
};
use serde::Deserialize;

use crate::{
    api::{
        client::{
            rpc_connection::{AuthorizationError, Authorize},
            session::WsSession,
        },
        control::{MemberId, RoomId},
    },
    conf::{Conf, Rpc},
    log::prelude::*,
    shutdown::ShutdownGracefully,
    signalling::room_repo::RoomRepository,
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
    request: HttpRequest,
    info: Path<RequestParams>,
    state: Data<Context>,
    payload: Payload,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    debug!("Request params: {:?}", info);

    match state.rooms.get(&info.room_id) {
        Some(room) => Either::A(
            room.send(Authorize {
                member_id: info.member_id.clone(),
                credentials: info.credentials.clone(),
            })
            .from_err()
            .and_then(move |res| match res {
                Ok(_) => ws::start(
                    WsSession::new(
                        info.member_id.clone(),
                        room,
                        state.config.idle_timeout,
                    ),
                    &request,
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
        None => Either::B(future::ok(HttpResponse::NotFound().into())),
    }
}

/// Context for [`App`] which holds all the necessary dependencies.
pub struct Context {
    /// Repository of all currently existing [`Room`]s in application.
    ///
    /// [`Room`]: crate::signalling::Room
    pub rooms: RoomRepository,

    /// Settings of application.
    pub config: Rpc,
}

/// HTTP server that handles WebSocket connections of Client API.
pub struct Server(ActixServer);

impl Server {
    /// Starts Client API HTTP server.
    pub fn run(rooms: RoomRepository, config: Conf) -> io::Result<Addr<Self>> {
        let server_addr = config.server.http.bind_addr();

        let server = HttpServer::new(move || {
            App::new()
                .data(Context {
                    rooms: rooms.clone(),
                    config: config.rpc.clone(),
                })
                .wrap(middleware::Logger::default())
                .service(
                    resource("/ws/{room_id}/{member_id}/{credentials}")
                        .route(actix_web::web::get().to_async(ws_index)),
                )
        })
        .disable_signals()
        .bind(server_addr)?
        .start();

        info!("Started Client API HTTP server on {}", server_addr);

        Ok(Self(server).start())
    }
}

impl Actor for Server {
    type Context = actix::Context<Self>;
}

impl Handler<ShutdownGracefully> for Server {
    type Result = ResponseFuture<(), ()>;

    fn handle(
        &mut self,
        _: ShutdownGracefully,
        _: &mut Self::Context,
    ) -> Self::Result {
        info!("Server received ShutdownGracefully message so shutting down");
        Box::new(self.0.stop(true))
    }
}

#[cfg(test)]
mod test {
    use std::{ops::Add, thread, time::Duration};

    use actix_http::{ws::Message, HttpService};
    use actix_http_test::{TestServer, TestServerRuntime};
    use futures::{future::IntoFuture as _, sink::Sink as _, Stream as _};

    use crate::{
        api::control, conf::Conf, signalling::Room,
        turn::new_turn_auth_service_mock, AppContext,
    };

    use super::*;

    /// Creates [`RoomRepository`] for tests filled with a single [`Room`].
    fn room(conf: Conf) -> RoomRepository {
        let room_spec =
            control::load_from_yaml_file("tests/specs/pub_sub_video_call.yml")
                .unwrap();

        let app = AppContext::new(conf, new_turn_auth_service_mock());

        let room_id = room_spec.id.clone();
        let client_room = Room::new(&room_spec, app.clone()).unwrap().start();
        let room_hash_map = hashmap! {
            room_id => client_room,
        };

        RoomRepository::new(room_hash_map)
    }

    /// Creates test WebSocket server of Client API which can handle requests.
    fn ws_server(conf: Conf) -> TestServerRuntime {
        TestServer::new(move || {
            HttpService::new(
                App::new()
                    .data(Context {
                        config: conf.rpc.clone(),
                        rooms: room(conf.clone()),
                    })
                    .service(
                        resource("/ws/{room_id}/{member_id}/{credentials}")
                            .route(actix_web::web::get().to_async(ws_index)),
                    ),
            )
        })
    }

    #[test]
    fn ping_pong_and_disconnects_on_idle() {
        let conf = Conf {
            rpc: Rpc {
                idle_timeout: Duration::new(2, 0),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut server = ws_server(conf.clone());
        let socket =
            server.ws_at("/ws/pub-sub-video-call/caller/test").unwrap();

        server
            .block_on(
                socket
                    .send(Message::Text(r#"{"ping": 33}"#.into()))
                    .into_future()
                    .map_err(|e| panic!("{:?}", e))
                    .and_then(|socket| {
                        socket
                            .into_future()
                            .map_err(|(e, _)| panic!("{:?}", e))
                            .and_then(|(item, read)| {
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
                            })
                    }),
            )
            .unwrap();
    }
}
