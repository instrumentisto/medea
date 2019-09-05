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
use derive_more::Display;
use failure::{Error, Fail};
use futures::future::{Either, Future, IntoFuture as _};
use std::collections::HashMap;

use crate::{
    api::{
        client::server::Server,
        control::{load_static_specs_from_dir, RoomId},
    },
    conf::Conf,
    log::prelude::*,
    shutdown::GracefulShutdown,
    signalling::{room::RoomError, room_repo::RoomsRepository, Room},
    turn::{service, TurnServiceErr},
};

/// Errors which can happen while server starting.
#[derive(Debug, Fail, Display)]
pub enum ServerStartError {
    /// Duplicate [`RoomId`] founded.
    #[display(fmt = "Duplicate of room ID '{:?}'", _0)]
    DuplicateRoomId(RoomId),

    /// Some error happened while loading spec.
    #[display(fmt = "Failed to load specs. {}", _0)]
    LoadSpec(failure::Error),

    /// Some error happened while creating new room from spec.
    #[display(fmt = "Bad room spec. {}", _0)]
    BadRoomSpec(String),

    /// Unexpected error returned from room.
    #[display(fmt = "Unknown room error.")]
    UnknownRoomError,
}

impl From<RoomError> for ServerStartError {
    fn from(err: RoomError) -> Self {
        match err {
            RoomError::BadRoomSpec(m) => Self::BadRoomSpec(m),
            _ => Self::UnknownRoomError,
        }
    }
}

pub fn run() -> Result<(), Error> {
    dotenv::dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init()?;

    let config = Conf::parse()?;
    info!("{:?}", config);

    actix::run(|| {
        start_static_rooms(&config)
            .map_err(|e| error!("Turn: {:?}", e))
            .map(Result::unwrap)
            .map(move |(res, graceful_shutdown)| {
                (res, graceful_shutdown, config)
            })
            .map(|(res, graceful_shutdown, config)| {
                let rooms = res;
                info!(
                    "Loaded rooms: {:?}",
                    rooms.iter().map(|(id, _)| &id.0).collect::<Vec<&String>>()
                );
                let room_repo = RoomsRepository::new(rooms);

                (room_repo, graceful_shutdown, config)
            })
            .and_then(|(room_repo, graceful_shutdown, config)| {
                Server::run(room_repo, config)
                    .map_err(|e| error!("Error starting server: {:?}", e))
                    .map(|server| {
                        graceful_shutdown
                            .send(shutdown::Subscribe(shutdown::Subscriber {
                                addr: server.recipient(),
                                priority: shutdown::Priority(1),
                            }))
                            .map_err(|e| error!("Shutdown sub: {}", e))
                            .map(|_| ())
                    })
                    .into_future()
                    .flatten()
            })
    })
    .unwrap();

    Ok(())
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
// TODO: temporary solution, changed in 32-grpc-dynamic-control-api branch
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
    let static_specs_path = config.control.static_specs_dir.clone();
    if let Ok(static_specs_dir) = std::fs::read_dir(&static_specs_path) {
        Either::A(service::new_turn_auth_service(&config.turn).map(
            move |turn_auth_service| {
                let room_specs =
                    match load_static_specs_from_dir(static_specs_dir) {
                        Ok(r) => r,
                        Err(e) => return Err(ServerStartError::LoadSpec(e)),
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
                    let rpc_reconnect_timeout = config.rpc.reconnect_timeout;
                    let turn_cloned = Arc::clone(&turn_auth_service);
                    let room = Room::start_in_arbiter(&arbiter, move |_| {
                        Room::new(&spec, rpc_reconnect_timeout, turn_cloned)
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
            },
        ))
    } else {
        warn!(
            "'{}' dir not found. Static Control API specs will not be loaded.",
            static_specs_path
        );
        Either::B(futures::future::ok(Ok((HashMap::new(), graceful_shutdown))))
    }
}
