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
    api::{client::server, control::load_static_specs_from_dir},
    conf::Conf,
    signalling::{room_repo::RoomsRepository, Room},
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

    let room_repo = RoomsRepository::new(start_static_rooms(&config));

    server::run(room_repo, config);
    let _ = sys.run();
}

/// Parses static [`Room`]s from config and starts them in separate arbiters.
fn start_static_rooms(config: &Conf) -> HashMap<String, Addr<Room>> {
    if let Some(static_specs_path) = config.server.static_specs_path.clone() {
        let room_specs = load_static_specs_from_dir(static_specs_path).unwrap();
        room_specs
            .into_iter()
            .map(|room_spec| {
                let room = Room::new(&room_spec, config.rpc.reconnect_timeout);
                let room_id = room.get_id();
                let room = Arbiter::start(move |_| room);

                (room_id, room)
            })
            .collect()
    } else {
        HashMap::new()
    }
}
