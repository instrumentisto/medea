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
                    WsSession::new(
                        member_id,
                        room,
                        state.config.idle_timeout,
                        state.config.ping_interval,
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
        let server_addr = config.server.client.http.bind_addr();

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

// TODO (evdokimovs): adapt tests from instrumentisto/medea#76 when they will be
//                    merged into master.
