//! Medea media server application.

#[macro_use]
mod utils;

mod api;
mod conf;
mod log;

use actix::prelude::*;
use dotenv::dotenv;
use hashbrown::HashMap;
use log::prelude::*;

use crate::{
    api::{
        client::{server, Room, RoomsRepository},
        control::Member,
    },
    conf::Conf,
};

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };
    let room = Arbiter::start(move |_| Room {
        id: 1,
        members,
        connections: HashMap::new(),
    });
    let rooms = hashmap! {1 => room};
    let rooms_repo = RoomsRepository::new(rooms);

    let config = Conf::parse().unwrap();

    info!("{:?}", config);

    server::run(rooms_repo, config);
    let _ = sys.run();
}
