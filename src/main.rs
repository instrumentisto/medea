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
    api::control::RoomId,
    api::{client::server, control::load_static_specs_from_dir},
    conf::Conf,
    signalling::{room_repo::RoomsRepository, Room},
    signalling::room::RoomError
};

use failure::Fail;
use hashbrown::HashMap;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let config = Conf::parse().unwrap();
    info!("{:?}", config);

    match start_static_rooms(&config) {
        Ok(r) => {
            let room_repo = RoomsRepository::new(r);
            server::run(room_repo, config);
        }
        Err(e) => {
            error!("Server not started because of error: '{}'", e);
            System::current().stop_with_code(100);
        }
    };

    let _ = sys.run();
}

/// Errors which can happen while server starting.
#[derive(Debug, Fail)]
enum ServerStartError {
    /// Duplicate room ID finded.
    #[fail(display = "Duplicate of room ID '{}'", _0)]
    DuplicateRoomId(RoomId),

    /// Some error happened while loading spec.
    #[fail(display = "Failed to load specs. {}", _0)]
    LoadSpec(failure::Error),

    /// Some error happened while creating new room from spec.
    #[fail(display = "Bad room spec. {}", _0)]
    BadRoomSpec(String),

    /// Unexpected error returned from room.
    #[fail(display = "Unknown room error.")]
    UnknownRoomError,
}

impl From<RoomError> for ServerStartError {
    fn from(err: RoomError) -> Self {
        match err {
            RoomError::BadRoomSpec(m) => ServerStartError::BadRoomSpec(m),
            _ => ServerStartError::UnknownRoomError,
        }
    }
}

/// Parses static [`Room`]s from config and starts them in separate arbiters.
///
/// Returns [`ServerStartError::DuplicateRoomId`] if find duplicated room ID.
///
/// Returns [`ServerStartError::LoadSpec`] if some error happened
/// while loading spec.
///
/// Returns [`ServerStartError::BadRoomSpec`]
/// if some error happened while creating room from spec.
fn start_static_rooms(
    config: &Conf,
) -> Result<HashMap<String, Addr<Room>>, ServerStartError> {
    if let Some(static_specs_path) = config.server.static_specs_path.clone() {
        let room_specs = match load_static_specs_from_dir(static_specs_path) {
            Ok(r) => r,
            Err(e) => return Err(ServerStartError::LoadSpec(e)),
        };
        let mut rooms = HashMap::new();

        for spec in room_specs {
            if rooms.contains_key(&spec.id) {
                return Err(ServerStartError::DuplicateRoomId(spec.id));
            }

            let room = Room::new(&spec, config.rpc.reconnect_timeout)?;
            let room = Arbiter::start(move |_| room);
            rooms.insert(spec.id, room);
        }

        Ok(rooms)
    } else {
        Ok(HashMap::new())
    }
}
