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

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;
    use futures::stream::Stream;
    use hashbrown::HashMap;

    use crate::api::control::*;

    fn test_members() -> HashMap<Id, Member> {
        hashmap! {
            1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
            2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
        }
    }

    #[test]
    fn connect_by_credentials() {
        let members_repo =
            Arc::new(Mutex::new(MemberRepository::new(test_members())));
        let session_repo = Arc::new(Mutex::new(WsSessionRepository::default()));

        let mut srv = test::TestServer::with_factory(move || {
            App::with_state(AppState {
                members_repo: members_repo.clone(),
                session_repo: session_repo.clone(),
            })
            .resource("/ws/{credentials}", |r| {
                r.method(http::Method::GET).with(ws_index)
            })
        });
        let (reader, mut writer) = srv.ws_at("/ws/caller_credentials").unwrap();

        writer.text("text");
        let (item, reader) = srv.execute(reader.into_future()).unwrap();
        assert_eq!(item, Some(ws::Message::Text("text".to_owned())));
    }
}
