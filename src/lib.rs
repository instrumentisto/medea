//! Medea media server application.

#[macro_use]
pub mod utils;
pub mod api;
pub mod conf;
pub mod log;
pub mod media;
pub mod signalling;
pub mod turn;

use actix::prelude::*;
use failure::Fail;
use hashbrown::HashMap;

use crate::{
    api::{control::load_static_specs_from_dir, control::RoomId},
    conf::Conf,
    signalling::{room::RoomError, Room},
    turn::new_turn_auth_service,
};

/// Errors which can happen while server starting.
#[derive(Debug, Fail)]
pub enum ServerStartError {
    /// Duplicate room ID finded.
    #[fail(display = "Duplicate of room ID '{:?}'", _0)]
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
pub fn start_static_rooms(
    config: &Conf,
) -> Result<HashMap<RoomId, Addr<Room>>, ServerStartError> {
    if let Some(static_specs_path) = &config.server.static_specs_path {
        let room_specs = match load_static_specs_from_dir(static_specs_path) {
            Ok(r) => r,
            Err(e) => return Err(ServerStartError::LoadSpec(e)),
        };
        let mut rooms = HashMap::new();

        for spec in room_specs {
            if rooms.contains_key(spec.id()) {
                return Err(ServerStartError::DuplicateRoomId(
                    spec.id().clone(),
                ));
            }

                let turn_auth_service = new_turn_auth_service(&config).expect("Unable to start turn service");

            let room = Room::new(&spec, config.rpc.reconnect_timeout,
                                 turn_auth_service)?;
            let room = Arbiter::start(move |_| room);
            rooms.insert(spec.id().clone(), room);
        }

        Ok(rooms)
    } else {
        Ok(HashMap::new())
    }
}
