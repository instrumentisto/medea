//! Server which provides API for upping and downing connection for `Member`.

use actix::{Actor as _, Addr, MailboxError};
use actix_cors::Cors;
use actix_web::{
    dev::Server, middleware, web, web::Data, App, HttpResponse, HttpServer,
    ResponseError,
};
use clap::ArgMatches;
use derive_more::{Display, From};
use futures::Future;
use iptables::error::IPTError;
use serde::Serialize;

use crate::{
    firewall::Firewall,
    gremlin::{Gremlin, Start, Stop},
};

#[derive(Display, Debug, From)]
pub enum ServerError {
    #[display(fmt = "Iptables error. {:?}", _0)]
    IptablesErr(IPTError),

    #[display(fmt = "Gremlin service error. {:?}", _0)]
    GremlinServiceErr(MailboxError),
}

impl ResponseError for ServerError {
    fn render_response(&self) -> HttpResponse {
        #[derive(Serialize)]
        struct ErrorResponse {
            error_message: String,
        }

        HttpResponse::InternalServerError().json(ErrorResponse {
            error_message: self.to_string(),
        })
    }
}

/// Runs [`actix::Server`] which will provide API for upping and downing
/// connections to `port_to_drop` port.
pub fn run(opts: ArgMatches) -> Server {
    let port_to_drop = opts.value_of("port").unwrap().parse().unwrap();

    let firewall = Firewall::new().unwrap();
    let gremlin = Gremlin::new(port_to_drop, firewall.clone()).start();

    HttpServer::new(move || {
        App::new()
            .data(Context {
                firewall: firewall.clone(),
                gremlin: gremlin.clone(),
                port_to_drop,
            })
            .wrap(Cors::new())
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/connection/up")
                    .route(web::post().to(up_connection)),
            )
            .service(
                web::resource("/connection/down")
                    .route(web::post().to(down_connection)),
            )
            .service(
                web::resource("/gremlin/start")
                    .route(web::post().to_async(start_gremlin)),
            )
            .service(
                web::resource("/gremlin/stop")
                    .route(web::post().to_async(stop_gremlin)),
            )
    })
    .bind(opts.value_of("addr").unwrap())
    .unwrap()
    .start()
}

/// Context of [`actix::Server`] which provide API for dropping connections.
pub struct Context {
    /// [`Firewall`] with which we can up/down `Member`'s connection.
    firewall: Firewall,

    /// Service which can randomly up/down connection for `Member`.
    gremlin: Addr<Gremlin>,

    /// Port which server will close/open on request.
    port_to_drop: u16,
}

/// Ups connection for `Member` with `iptables`.
///
/// `POST /connection/up`
#[allow(clippy::needless_pass_by_value)]
pub fn up_connection(
    state: Data<Context>,
) -> Result<HttpResponse, ServerError> {
    state.firewall.open_port(state.port_to_drop)?;
    Ok(HttpResponse::Ok().finish())
}

/// Drops connection for `Member` with `iptables`.
///
/// `POST /connection/down`
#[allow(clippy::needless_pass_by_value)]
pub fn down_connection(
    state: Data<Context>,
) -> Result<HttpResponse, ServerError> {
    state.firewall.close_port(state.port_to_drop)?;
    Ok(HttpResponse::Ok().finish())
}

/// Starts service which will up/down connection for `Member` at random time.
///
/// `POST /gremlin/start`
#[allow(clippy::needless_pass_by_value)]
pub fn start_gremlin<'a>(
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ServerError> {
    state.gremlin.send(Start).from_err().and_then(|res| {
        res?;
        Ok(HttpResponse::Ok().finish())
    })
}

/// Stops service which will up/down connection for `Member` at random time.
///
/// `POST /gremlin/stop`
#[allow(clippy::needless_pass_by_value)]
pub fn stop_gremlin(
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ServerError> {
    state.gremlin.send(Stop).from_err().and_then(|res| {
        res?;
        Ok(HttpResponse::Ok().finish())
    })
}
