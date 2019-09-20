use crate::{
    firewall::Firewall,
    gremlin::{Gremlin, Start, Stop},
};
use actix::Addr;
use actix_web::{
    dev::Server, error::PayloadError::Http2Payload, middleware, web, web::Data,
    App, HttpResponse, HttpServer,
};
use futures::{future, Future};
use iptables::{error::IPTError, IPTables};
use serde::Serialize;
use std::borrow::Cow;

pub fn run(firewall: Firewall, gremlin: Addr<Gremlin>) -> Server {
    HttpServer::new(move || {
        App::new()
            .data(Context {
                firewall: firewall.clone(),
                gremlin: gremlin.clone(),
            })
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

pub struct Context {
    firewall: Firewall,
    gremlin: Addr<Gremlin>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse<'a> {
    error_text: Cow<'a, str>,
}

impl<'a> ErrorResponse<'a> {
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

#[allow(clippy::needless_pass_by_value)]
pub fn down_connection(
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    match state.firewall.close_port(8090) {
        Ok(_) => future::ok(HttpResponse::Ok().finish()),
        Err(e) => future::ok(ErrorResponse::from(e).into()),
    }
}

#[allow(clippy::needless_pass_by_value)]
pub fn start_gremlin(state: Data<Context>) -> HttpResponse {
    state.gremlin.do_send(Start);
    HttpResponse::Ok().finish()
}

#[allow(clippy::needless_pass_by_value)]
pub fn stop_gremlin(state: Data<Context>) -> HttpResponse {
    state.gremlin.do_send(Stop);
    HttpResponse::Ok().finish()
}
