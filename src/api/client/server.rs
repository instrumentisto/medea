//! HTTP server for handling WebSocket connections of Client API.

use actix_web::{
    http, middleware, server, ws, App, Error, HttpRequest, HttpResponse, Path,
    State,
};

use crate::{
    api::{
        client::session::{WsSession, WsSessionContext, WsSessionRepository},
        control::member::MemberRepository,
    },
    log::prelude::*,
};

/// Handles all HTTP requests, performs WebSocket handshake (upgrade) and starts
/// new [`WsSessions`] actor for WebSocket connection.
fn ws_index(
    (r, creds, state): (
        HttpRequest<WsSessionContext>,
        Path<String>,
        State<WsSessionContext>,
    ),
) -> Result<HttpResponse, Error> {
    match state.members.get_by_credentials(creds.as_str()) {
        Some(member) => ws::start(&r, WsSession::new(member.id)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

/// Starts HTTP server for handling WebSocket connections.
pub fn run(members_repo: MemberRepository) {
    let session_repo = WsSessionRepository::default();

    server::new(move || {
        App::with_state(WsSessionContext {
            members: members_repo.clone(),
            sessions: session_repo.clone(),
        })
        .middleware(middleware::Logger::default())
        .resource("/ws/{credentials}", |r| {
            r.method(http::Method::GET).with(ws_index)
        })
    })
    .bind("0.0.0.0:8080")
    .unwrap()
    .start();

    info!("Started HTTP server on 0.0.0.0:8080");
}
