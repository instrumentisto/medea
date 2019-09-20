//! Server which provides API for upping and downing connection for `Member`.

use std::borrow::Cow;

use actix::Addr;
use actix_cors::Cors;
use actix_web::{
    dev::Server, middleware, web, web::Data, App, HttpResponse, HttpServer,
};
use futures::{future, Future};
use iptables::error::IPTError;
use serde::Serialize;

use crate::{
    firewall::Firewall,
    gremlin::{Gremlin, Start, Stop},
    prelude::*,
};

/// Runs [`actix::Server`] which will provide API for upping and downing
/// connection for `Member`.
pub fn run(firewall: Firewall, gremlin: Addr<Gremlin>) -> Server {
    HttpServer::new(move || {
        App::new()
            .data(Context {
                firewall: firewall.clone(),
                gremlin: gremlin.clone(),
            })
            .wrap(Cors::new())
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/connection/up")
                    .route(web::post().to_async(up_connection)),
            )
            .service(
                web::resource("/connection/down")
                    .route(web::post().to_async(down_connection)),
            )
            .service(
                web::resource("/gremlin/start")
                    .route(web::post().to(start_gremlin)),
            )
            .service(
                web::resource("/gremlin/stop")
                    .route(web::post().to(stop_gremlin)),
            )
    })
    .bind("127.0.0.1:8500")
    .unwrap()
    .start()
}

/// Context of [`actix::Server`] which provide API for dropping connections.
pub struct Context {
    firewall: Firewall,
    gremlin: Addr<Gremlin>,
}

/// Error response.
#[derive(Debug, Serialize)]
struct ErrorResponse<'a> {
    /// Text of error.
    error_text: Cow<'a, str>,
}

impl<'a> ErrorResponse<'a> {
    /// Create new [`ErrorResponse`] with provided text as error text.
    pub fn new<S>(text: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        Self {
            error_text: text.into(),
        }
    }
}

impl<'a> From<IPTError> for ErrorResponse<'a> {
    fn from(err: IPTError) -> Self {
        ErrorResponse::new(err.to_string())
    }
}

impl<'a> Into<HttpResponse> for ErrorResponse<'a> {
    fn into(self) -> HttpResponse {
        HttpResponse::InternalServerError().json(self)
    }
}

/// Ups connection for `Member` with `iptables`.
///
/// `POST /connection/up`
#[allow(clippy::needless_pass_by_value)]
pub fn up_connection(
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    match state.firewall.open_port(8090) {
        Ok(is_deleted) => {
            if is_deleted {
                future::ok(HttpResponse::Ok().finish())
            } else {
                future::ok(ErrorResponse::new("Nothing deleted.").into())
            }
        }
        Err(e) => future::ok(ErrorResponse::from(e).into()),
    }
}

/// Drops connection for `Member` with `iptables`.
///
/// `POST /connection/down`
#[allow(clippy::needless_pass_by_value)]
pub fn down_connection(
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    match state.firewall.close_port(8090) {
        Ok(_) => future::ok(HttpResponse::Ok().finish()),
        Err(e) => match e {
            IPTError::Other(s) => {
                warn!("Ignored iptables error: {}", s);
                future::ok(HttpResponse::Ok().finish())
            }
            _ => future::ok(ErrorResponse::from(e).into()),
        },
    }
}

/// Starts service which will up/down connection for `Member` at random time.
///
/// `POST /gremlin/start`
#[allow(clippy::needless_pass_by_value)]
pub fn start_gremlin(state: Data<Context>) -> HttpResponse {
    state.gremlin.do_send(Start);
    HttpResponse::Ok().finish()
}

/// Stops service which will up/down connection for `Member` at random time.
///
/// `POST /gremlin/stop`
#[allow(clippy::needless_pass_by_value)]
pub fn stop_gremlin(state: Data<Context>) -> HttpResponse {
    state.gremlin.do_send(Stop);
    HttpResponse::Ok().finish()
}
