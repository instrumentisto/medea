//! Implementation HTTP server for handle websocket connections.

use actix_web::{
    http, middleware, server, ws, App, Error, HttpRequest, HttpResponse, Path,
    State,
};

use crate::{
    api::client::session::{WsSessionRepository, WsSessionState, WsSessions},
    api::control::member::MemberRepository,
    log::prelude::*,
};

/// Do websocket handshake and start `WsSessions` actor
fn ws_index(
    (r, creds, state): (
        HttpRequest<WsSessionState>,
        Path<String>,
        State<WsSessionState>,
    ),
) -> Result<HttpResponse, Error> {
    match state.members_repo.get_by_credentials(creds.as_str()) {
        Some(member) => ws::start(&r, WsSessions::new(member.id)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

/// Starts HTTP server for handle websocket upgrade request.
pub fn run(members_repo: MemberRepository) {
    let session_repo = WsSessionRepository::default();

    server::new(move || {
        App::with_state(WsSessionState {
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
