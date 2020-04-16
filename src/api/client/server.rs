//! HTTP server for handling WebSocket connections of Client API.

use std::io;

use actix::{Actor, Addr, Handler};
use actix_web::{
    dev::Server as ActixServer,
    middleware,
    web::{resource, Data, Path, Payload, ServiceConfig},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::ws;
use futures::FutureExt as _;
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
    utils::ResponseAnyFuture,
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
async fn ws_index(
    request: HttpRequest,
    info: Path<RequestParams>,
    state: Data<Context>,
    payload: Payload,
) -> Result<HttpResponse, actix_web::Error> {
    debug!("Request params: {:?}", info);
    let RequestParams {
        room_id,
        member_id,
        credentials,
    } = info.into_inner();

    match state.rooms.get(&room_id) {
        Some(room) => {
            let auth_result = room
                .send(Authorize {
                    member_id: member_id.clone(),
                    credentials,
                })
                .await?;
            match auth_result {
                Ok(settings) => ws::start(
                    WsSession::new(
                        member_id,
                        room_id,
                        Box::new(room),
                        settings.idle_timeout,
                        settings.ping_interval,
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
            }
        }
        None => Ok(HttpResponse::NotFound().into()),
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
    ///
    /// # Errors
    ///
    /// Errors if binding [`HttpServer`] to a listening address fails.
    pub fn run(rooms: RoomRepository, config: Conf) -> io::Result<Addr<Self>> {
        let server_addr = config.server.client.http.bind_addr();

        let server = HttpServer::new(move || {
            App::new()
                .app_data(Self::app_data(rooms.clone(), config.rpc))
                .configure(Self::configure)
                .wrap(middleware::Logger::default())
        })
        .disable_signals()
        .bind(server_addr)?
        .run();

        info!("Started Client API HTTP server on {}", server_addr);

        Ok(Self(server).start())
    }

    /// Set application data.
    fn app_data(rooms: RoomRepository, config: Rpc) -> Data<Context> {
        Data::new(Context { rooms, config })
    }

    /// Run external configuration as part of the application building
    /// process
    fn configure(cfg: &mut ServiceConfig) {
        cfg.service(
            resource("/ws/{room_id}/{member_id}/{credentials}")
                .route(actix_web::web::get().to(ws_index)),
        );
    }
}

impl Actor for Server {
    type Context = actix::Context<Self>;
}

impl Handler<ShutdownGracefully> for Server {
    type Result = ResponseAnyFuture<()>;

    fn handle(
        &mut self,
        _: ShutdownGracefully,
        _: &mut Self::Context,
    ) -> Self::Result {
        info!("Server received ShutdownGracefully message so shutting down");
        ResponseAnyFuture(self.0.stop(true).boxed_local())
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use actix_web::test::TestServer;
    use awc::error::WsClientError;

    use crate::{
        api::control,
        conf::Conf,
        signalling::{peers::build_peers_traffic_watcher, Room},
        turn::new_turn_auth_service_mock,
        AppContext,
    };

    use super::*;

    /// Creates [`RoomRepository`] for tests filled with a single [`Room`].
    fn build_room_repo(conf: Conf) -> RoomRepository {
        let room_spec =
            control::load_from_yaml_file("tests/specs/pub-sub-video-call.yml")
                .unwrap();

        let traffic_watcher =
            build_peers_traffic_watcher(&conf.peer_media_traffic);
        let app = AppContext::new(conf, new_turn_auth_service_mock());

        let room_id = room_spec.id.clone();
        let client_room = Room::new(&room_spec, &app, traffic_watcher)
            .unwrap()
            .start();
        let room_hash_map = hashmap! {
            room_id => client_room,
        };

        RoomRepository::new(room_hash_map)
    }

    /// Creates test WebSocket server of Client API which can handle requests.
    fn ws_server(conf: Conf) -> TestServer {
        actix_web::test::start(move || {
            App::new()
                .app_data(Server::app_data(
                    build_room_repo(conf.clone()),
                    conf.rpc,
                ))
                .configure(Server::configure)
        })
    }

    #[actix_rt::test]
    async fn forbidden_if_bad_credentials() {
        let conf = Conf {
            rpc: Rpc {
                idle_timeout: Duration::new(1, 0),
                ..Rpc::default()
            },
            ..Conf::default()
        };

        let mut server = ws_server(conf.clone());
        match server
            .ws_at("/ws/pub-sub-video-call/caller/bad_credentials")
            .await
        {
            Err(WsClientError::InvalidResponseStatus(code)) => {
                assert_eq!(code, 403);
            }
            _ => unreachable!(),
        }
    }

    #[actix_rt::test]
    async fn not_found_if_bad_url() {
        let conf = Conf {
            rpc: Rpc {
                idle_timeout: Duration::new(1, 0),
                ..Rpc::default()
            },
            ..Conf::default()
        };

        let mut server = ws_server(conf.clone());
        match server.ws_at("/ws/bad_room/caller/test").await {
            Err(WsClientError::InvalidResponseStatus(code)) => {
                assert_eq!(code, 404);
            }
            _ => unreachable!(),
        };
        match server.ws_at("/ws/pub-sub-video-call/bad_member/test").await {
            Err(WsClientError::InvalidResponseStatus(code)) => {
                assert_eq!(code, 404);
            }
            _ => unreachable!(),
        };
    }

    #[actix_rt::test]
    async fn established() {
        let conf = Conf {
            rpc: Rpc {
                idle_timeout: Duration::new(1, 0),
                ..Rpc::default()
            },
            ..Conf::default()
        };

        let mut server = ws_server(conf.clone());
        server
            .ws_at("/ws/pub-sub-video-call/caller/test")
            .await
            .unwrap();
    }
}
