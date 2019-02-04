//! Implementation HTTP server for handle websocket connections.

use std::sync::{Arc, Mutex};

use actix_web::{
    http, middleware, server, ws, App, Error, HttpRequest, HttpResponse, Path,
    State,
};

use crate::{
    api::client::session::{WsSessionRepository, WsSessions},
    api::control::member::{Member, MemberRepository},
    log::prelude::*,
};

/// Do websocket handshake and start `WsSessions` actor
fn ws_index(
    (r, creds, state): (HttpRequest<AppState>, Path<String>, State<AppState>),
) -> Result<HttpResponse, Error> {
    let member_repo = state.members_repo.lock().unwrap();
    match member_repo.get_by_credentials(creds.as_str()) {
        Some(member) => ws::start(&r, WsSessions::new(member.id)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

/// State with repositories addresses
pub struct AppState {
    pub members_repo: Arc<Mutex<MemberRepository>>,
    pub session_repo: Arc<Mutex<WsSessionRepository>>,
}

pub fn run(members_repo: Arc<Mutex<MemberRepository>>) {
    let session_repo = Arc::new(Mutex::new(WsSessionRepository::default()));

    server::new(move || {
        App::with_state(AppState {
            members_repo: members_repo.clone(),
            session_repo: session_repo.clone(),
        })
        .middleware(middleware::Logger::default())
        .resource("/ws/{credentials}", |r| {
            r.method(http::Method::GET).with(ws_index)
        })
    })
    .bind("127.0.0.1:8080")
    .unwrap()
    .start();

    info!("Started http server: 127.0.0.1:8080");
}
