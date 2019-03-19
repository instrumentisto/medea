//! Medea media server application.

#[macro_use]
extern crate lazy_static;

use actix::prelude::*;
use dotenv::dotenv;

use crate::api::{
    client::{server, Room, RoomsRepository},
    control::Member,
};

#[macro_use]
mod utils;

mod api;
mod log;
mod media;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let members = hashmap! {
        1 => Member{id: 1, credentials: "responder_credentials".to_owned()},
        2 => Member{id: 2, credentials: "caller_credentials".to_owned()},
    };
    let room = Arbiter::start(move |_| Room::new(1, members));
    let rooms = hashmap! {1 => room};
    let rooms_repo = RoomsRepository::new(rooms);

    server::run(rooms_repo);
    let _ = sys.run();
}
