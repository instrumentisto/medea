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
        None => Ok(HttpResponse::NotFound().finish()),
        Some(member) => {
            let session_repo = state.session_repo.lock().unwrap();
            if session_repo.is_connected(member.id) {
                Ok(HttpResponse::Conflict().finish())
            } else {
                ws::start(&r, WsSessions::new(member.id))
            }
        }
    }
}

/// State with repositories addresses
pub struct AppState {
    pub members_repo: Arc<Mutex<MemberRepository>>,
    pub session_repo: Arc<Mutex<WsSessionRepository>>,
}

pub fn run() {
    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };

    let members_repo = Arc::new(Mutex::new(MemberRepository::new(members)));
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
