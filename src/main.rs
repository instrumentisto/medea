//! Medea media server application.

#[macro_use]
extern crate macro_attr;
#[macro_use]
extern crate newtype_derive;

#[macro_use]
mod utils;

pub mod api;
pub mod conf;
pub mod log;
pub mod media;

use actix::prelude::*;
use dotenv::dotenv;
use log::prelude::*;

use crate::{
    api::{
        client::{server, Room, RoomsRepository},
        control::Member,
    },
    conf::Conf,
    media::peer::create_peers,
};

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let config = Conf::parse().unwrap();

    info!("{:?}", config);

    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };
    let peers = create_peers(1, 2);
    let room = Room::new(1, members, peers, config.rpc.reconnect_timeout);
    let room = Arbiter::start(move |_| room);
    let rooms = hashmap! {1 => room};
    let rooms_repo = RoomsRepository::new(rooms);

    server::run(rooms_repo, config);
    let _ = sys.run();
}
