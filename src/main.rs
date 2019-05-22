//! Medea media server application.

#[macro_use]
pub mod utils;
pub mod api;
pub mod conf;
pub mod log;
pub mod media;
pub mod signalling;

use actix::prelude::*;
use dotenv::dotenv;
use log::prelude::*;

use crate::{
    api::{
        client::server,
        control::Member,
        control::{load_from_file, RoomRequest},
    },
    conf::Conf,
    media::create_peers,
    signalling::{Room, room_repo::RoomsRepository},
};
use hashbrown::HashMap;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let config = Conf::parse().unwrap();
    info!("{:?}", config);

    let room_spec = load_from_file("room_spec.yml").unwrap();

    println!("{:#?}", room_spec);

    let client_room = Room::new(room_spec, config.rpc.reconnect_timeout);
    let room_id = client_room.get_id();
    let client_room = Arbiter::start(move |_| client_room);
    let room_hash_map = hashmap! {
        room_id => client_room,
    };

    let room_repo = RoomsRepository::new(room_hash_map);

    server::run(room_repo, config);
    let _ = sys.run();
}
