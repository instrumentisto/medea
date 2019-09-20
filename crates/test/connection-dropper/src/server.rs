//! Server which provides API for upping and downing connection for `Member`.

use std::borrow::Cow;

use actix::Addr;
use actix_cors::Cors;
use actix_web::{
    dev::Server, middleware, web, web::Data, App, HttpResponse, HttpServer,
};
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
                    .route(web::post().to(up_connection)),
            )
            .service(
                web::resource("/connection/down")
                    .route(web::post().to(down_connection)),
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
    /// [`Firewall`] with which we can up/down `Member`'s connection.
    firewall: Firewall,

    /// Service which can randomly up/down connection for `Member`.
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
        Self::new(err.to_string())
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
pub fn up_connection(state: Data<Context>) -> HttpResponse {
    match state.firewall.open_port(8090) {
        Ok(is_deleted) => {
            if is_deleted {
                HttpResponse::Ok().finish()
            } else {
                ErrorResponse::new("Nothing deleted.").into()
            }
        }
        Err(e) => ErrorResponse::from(e).into(),
    }
}

/// Drops connection for `Member` with `iptables`.
///
/// `POST /connection/down`
#[allow(clippy::needless_pass_by_value)]
pub fn down_connection(state: Data<Context>) -> HttpResponse {
    match state.firewall.close_port(8090) {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => match e {
            IPTError::Other(s) => {
                warn!("Ignored iptables error: {}", s);
                HttpResponse::Ok().finish()
            }
            _ => ErrorResponse::from(e).into(),
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
