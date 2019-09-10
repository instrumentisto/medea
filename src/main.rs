use std::collections::HashMap;

use actix::{Actor, Addr};
use failure::Error;
use futures::future::Future;
use medea::{
    api::{
        client::server::Server,
        control::{grpc, LoadStaticControlSpecsError},
    },
    conf::Conf,
    log::{self, prelude::*},
    shutdown::{self, GracefulShutdown},
    signalling::{
        room_repo::RoomRepository,
        room_service::{RoomService, RoomServiceError, StartStaticRooms},
    },
    turn::new_turn_auth_service,
    AppContext,
};

fn start_static_rooms(
    room_service: &Addr<RoomService>,
) -> impl Future<Item = (), Error = ()> {
    room_service
        .send(StartStaticRooms)
        .map_err(|e| error!("StartStaticRooms mailbox error: {:?}", e))
        .map(|result| {
            if let Err(e) = result {
                match e {
                    RoomServiceError::FailedToLoadStaticSpecs(e) => match e {
                        LoadStaticControlSpecsError::SpecDirReadError(e) => {
                            warn!(
                                "Error while reading static control API specs \
                                 dir. Control API specs not loaded. {}",
                                e
                            );
                        }
                        _ => panic!("{}", e),
                    },
                    _ => panic!("{}", e),
                }
            }
        })
}

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init()?;

    let config = Conf::parse()?;
    info!("{:?}", config);

    actix::run(move || {
        new_turn_auth_service(&config.turn)
            .map_err(move |e| error!("{:?}", e))
            .map(|turn_service| {
                let graceful_shutdown =
                    GracefulShutdown::new(config.shutdown.timeout).start();
                (turn_service, graceful_shutdown, config)
            })
            .and_then(move |(turn_service, graceful_shutdown, config)| {
                let app_context = AppContext::new(config.clone(), turn_service);

                let room_repo = RoomRepository::new(HashMap::new());
                let room_service = RoomService::new(
                    room_repo.clone(),
                    app_context.clone(),
                    graceful_shutdown.clone(),
                )
                .start();

                start_static_rooms(&room_service).map(move |_| {
                    let grpc_addr =
                        grpc::server::run(room_service, app_context);
                    shutdown::subscribe(
                        &graceful_shutdown,
                        grpc_addr.clone().recipient(),
                        shutdown::Priority(1),
                    );

                    let server = Server::run(room_repo, config).unwrap();
                    shutdown::subscribe(
                        &graceful_shutdown,
                        server.recipient(),
                        shutdown::Priority(1),
                    );
                })
            })
    })
    .unwrap();

    Ok(())
}
