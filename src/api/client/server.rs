//! HTTP server for handling WebSocket connections of Client API.

use std::io;

use actix::{Actor, Addr, Handler, ResponseFuture};
use actix_web::{
    dev::Server as ActixServer,
    middleware,
    web::{resource, Data, Payload, ServiceConfig},
    App, HttpRequest, HttpResponse, HttpServer,
};
use actix_web_actors::{ws, ws::WebsocketContext};
use futures::FutureExt as _;

use crate::{
    api::client::session::WsSession,
    conf::{Conf, Rpc},
    log::prelude::*,
    shutdown::ShutdownGracefully,
    signalling::room_repo::RoomRepository,
};

use super::MAX_WS_MSG_SIZE;

/// Handles all HTTP requests, performs WebSocket handshake (upgrade) and starts
/// new [`WsSession`] for WebSocket connection.
async fn ws_index(
    request: HttpRequest,
    state: Data<Context>,
    payload: Payload,
) -> actix_web::Result<HttpResponse> {
    Ok(
        ws::handshake(&request)?.streaming(WebsocketContext::with_codec(
            WsSession::new(
                Box::new(state.rooms.clone()),
                state.config.idle_timeout,
                state.config.ping_interval,
            ),
            payload,
            actix_http::ws::Codec::new().max_size(MAX_WS_MSG_SIZE),
        )),
    )
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
        cfg.service(resource("/ws").route(actix_web::web::get().to(ws_index)));
    }
}

impl Actor for Server {
    type Context = actix::Context<Self>;
}

impl Handler<ShutdownGracefully> for Server {
    type Result = ResponseFuture<()>;

    fn handle(
        &mut self,
        _: ShutdownGracefully,
        _: &mut Self::Context,
    ) -> Self::Result {
        info!("Server received ShutdownGracefully message so shutting down");
        self.0.stop(true).boxed_local()
    }
}
