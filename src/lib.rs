//! Medea media server application.

#[macro_use]
pub mod utils;
pub mod api;
pub mod conf;
pub mod log;
pub mod media;
pub mod shutdown;
pub mod signalling;
pub mod turn;

use std::sync::Arc;

use actix::prelude::*;
use failure::Fail;
use std::collections::HashMap;

use crate::{
    api::control::{load_static_specs_from_dir, RoomId},
    conf::Conf,
    shutdown::GracefulShutdown,
    signalling::{room::RoomError, Room},
    turn::{service, TurnServiceErr},
};
use futures::future::Either;

/// Errors which can happen while server starting.
#[derive(Debug, Fail)]
pub enum ServerStartError {
    /// Duplicate [`RoomId`] founded.
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
// This is not the most beautiful solution, but at the moment let it be. In the
// 32-grpc-dynamic-control-api branch, this logic is changed and everything will
// look better.
pub fn start_static_rooms(
    conf: &Conf,
) -> impl Future<
    Item = Result<
        (HashMap<RoomId, Addr<Room>>, Addr<GracefulShutdown>),
        ServerStartError,
    >,
    Error = TurnServiceErr,
> {
    let graceful_shutdown =
        GracefulShutdown::new(conf.shutdown.timeout).start();
    let config = conf.clone();
    if let Some(static_specs_path) = config.server.static_specs_path.clone() {
        Either::A(
            service::new_turn_auth_service(&config.turn)
                .map(Arc::new)
                .map(move |turn_auth_service| {
                    let room_specs =
                        match load_static_specs_from_dir(static_specs_path) {
                            Ok(r) => r,
                            Err(e) => {
                                return Err(ServerStartError::LoadSpec(e))
                            }
                        };
                    let mut rooms = HashMap::new();
                    let arbiter = Arbiter::new();

                    for spec in room_specs {
                        if rooms.contains_key(spec.id()) {
                            return Err(ServerStartError::DuplicateRoomId(
                                spec.id().clone(),
                            ));
                        }

                        let room_id = spec.id().clone();
                        let rpc_reconnect_timeout =
                            config.rpc.reconnect_timeout;
                        let turn_cloned = Arc::clone(&turn_auth_service);
                        let room =
                            Room::start_in_arbiter(&arbiter, move |_| {
                                Room::new(
                                    &spec,
                                    rpc_reconnect_timeout,
                                    turn_cloned,
                                )
                                .unwrap()
                            });
                        graceful_shutdown.do_send(shutdown::Subscribe(
                            shutdown::Subscriber {
                                addr: room.clone().recipient(),
                                priority: shutdown::Priority(2),
                            },
                        ));
                        rooms.insert(room_id, room);
                    }

                    Ok((rooms, graceful_shutdown))
                }),
        )
    } else {
        Either::B(futures::future::ok(Ok((HashMap::new(), graceful_shutdown))))
    }
}
