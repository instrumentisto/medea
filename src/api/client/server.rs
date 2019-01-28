use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web::{
    http, middleware, server, ws, App, AsyncResponder, Error, FutureResponse,
    HttpRequest, HttpResponse, Path,
};
use futures::future::Future;

use crate::{
    api::client::*,
    api::control::member::{
        ControlError, GetMemberByCredentials, Id, Member, MemberRepository,
    },
    log::prelude::*,
};

/// do websocket handshake and start `WsSessions` actor
fn ws_index(
    (r, creds): (HttpRequest<()>, Path<String>),
) -> FutureResponse<HttpResponse> {
    info!("{:?}", creds);
    MemberRepository::from_registry()
        .send(GetMemberByCredentials(creds.into_inner()))
        .from_err()
        .and_then(|res| match res {
            Ok(res) => {
                info!("{:?}", res);
                WsSessionRepository::from_registry()
                    .send(IsConnected(res.id))
                    .from_err()
                    .and_then(move |is_connected| {
                        info!("{:?}", is_connected);
                        if is_connected {
                            return Ok(HttpResponse::Conflict().into());
                        }
                        ws::start(&r, WsSessions::new(res.id))
                    })
                    .wait()
            }
            Err(e) => match e {
                ControlError::NotFound => Ok(HttpResponse::NotFound().into()),
                _ => Ok(HttpResponse::InternalServerError().into()),
            },
        })
        .responder()
}

fn index(r: &HttpRequest<()>) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().finish())
}

/// State with repository address
#[derive(Debug)]
pub struct AppState {
    members_repo: Addr<MemberRepository>,
}

pub fn run() {
    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };

    let addr = Arbiter::start(move |_| MemberRepository { members });
    System::current().registry().set(addr);
    let addr2 = Arbiter::start(|_| WsSessionRepository::default());
    System::current().registry().set(addr2);

    server::new(move || {
        App::new()
            .middleware(middleware::Logger::default())
            .resource("/ws/{credentials}", |r| {
                r.method(http::Method::GET).with(ws_index)
            })
            .resource("/get/", |r| r.method(http::Method::GET).f(index))
    })
    .bind("127.0.0.1:8080")
    .unwrap()
    .start();

    info!("Started http server: 127.0.0.1:8080");
}
