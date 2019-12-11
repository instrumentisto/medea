//! HTTP server for handling WebSocket connections of Client API.

use std::io;

use actix::{Actor, Addr, Handler, ResponseFuture};
use actix_cors::Cors;
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
use actix_web::web::Json;

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
    let RequestParams {
        room_id,
        member_id,
        credentials,
    } = info.into_inner();

    match state.rooms.get(&room_id) {
        Some(room) => Either::A(
            room.send(Authorize {
                member_id: member_id.clone(),
                credentials,
            })
            .from_err()
            .and_then(move |res| match res {
                Ok(_) => ws::start(
                    WsSession::new(member_id, room, state.config.idle_timeout),
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

/// Handles POST `/logs` HTTP requests and logs body.
#[allow(clippy::needless_pass_by_value)]
fn log_index(body: Json<Vec<String>>) -> HttpResponse {
    for log in body.into_inner() {
        info!("client log: {}", log);
    }

    HttpResponse::Ok().body("Ok")
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
        let server_addr = config.server.client.http.bind_addr();

        let server = HttpServer::new(move || {
            App::new()
                .data(Context {
                    rooms: rooms.clone(),
                    config: config.rpc.clone(),
                })
                .wrap(middleware::Logger::default())
                .wrap(
                    Cors::new()
                        .send_wildcard()
                        .allowed_methods(vec!["POST"])
                        .max_age(3600),
                )
                .service(
                    resource("/ws/{room_id}/{member_id}/{credentials}")
                        .route(actix_web::web::get().to_async(ws_index)),
                )
                .service(
                    resource("/logs")
                        .route(actix_web::web::post().to(log_index)),
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

    use actix_http::{http::StatusCode, ws::Message, HttpService};
    use actix_http_test::{TestServer, TestServerRuntime};
    use actix_web::web::Bytes;
    use futures::{future::IntoFuture as _, sink::Sink as _, Stream as _};
    use medea_client_api_proto::{CloseDescription, CloseReason};

    use crate::{
        api::control, conf::Conf, signalling::Room,
        turn::new_turn_auth_service_mock, AppContext,
    };

    use super::*;

    /// Creates [`RoomRepository`] for tests filled with a single [`Room`].
    fn room(conf: Conf) -> RoomRepository {
        let room_spec =
            control::load_from_yaml_file("tests/specs/pub-sub-video-call.yml")
                .unwrap();

        let app = AppContext::new(conf, new_turn_auth_service_mock());

        let room_id = room_spec.id.clone();
        let client_room = Room::new(&room_spec, &app).unwrap().start();
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
                ..Rpc::default()
            },
            ..Conf::default()
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
                                        let description = CloseDescription::new(
                                            CloseReason::Idle,
                                        );
                                        let close_reason = ws::CloseReason {
                                            code: ws::CloseCode::Normal,
                                            description: Some(
                                                serde_json::to_string(
                                                    &description,
                                                )
                                                .unwrap(),
                                            ),
                                        };

                                        assert_eq!(
                                            Some(ws::Frame::Close(Some(
                                                close_reason
                                            ))),
                                            item
                                        );
                                    })
                            })
                    }),
            )
            .unwrap();
    }

    fn http_server() -> TestServerRuntime {
        TestServer::new(|| {
            HttpService::new(App::new().service(
                resource("/logs").route(actix_web::web::post().to(log_index)),
            ))
        })
    }

    #[test]
    fn bad_request_if_content_type_not_json() {
        let mut server = http_server();
        let req = server
            .post("/logs")
            .header("Content-Type", "text/plain")
            .send_body("test_log");
        let response = server.block_on(req).unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn bad_request_if_content_not_json() {
        let mut server = http_server();
        let req = server
            .post("/logs")
            .header("Content-Type", "application/json")
            .send_body("test_log");
        let mut response = server.block_on(req).unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = server.block_on(response.body()).unwrap();
        assert_eq!(
            bytes,
            Bytes::from_static(
                "Json deserialize error: expected ident at line 1 column 2"
                    .as_ref()
            )
        );
    }

    #[test]
    fn bad_request_if_content_not_vec_of_string() {
        let mut server = http_server();
        let req = server
            .post("/logs")
            .header("Content-Type", "application/json")
            .send_body("{\"log\":\"test_log\"}");
        let mut response = server.block_on(req).unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = server.block_on(response.body()).unwrap();
        assert_eq!(
            bytes,
            Bytes::from_static(
                "Json deserialize error: invalid type: map, expected a \
                 sequence at line 1 column 0"
                    .as_ref()
            )
        );
    }

    #[test]
    fn success_receive_json_vec_of_string() {
        let mut server = http_server();
        let req = server
            .post("/logs")
            .header("Content-Type", "application/json")
            .send_body("[\"test_log\",\"test_log2\"]");
        let mut response = server.block_on(req).unwrap();
        assert!(response.status().is_success());
        let bytes = server.block_on(response.body()).unwrap();
        assert_eq!(bytes, Bytes::from_static("Ok".as_ref()));
    }
}
