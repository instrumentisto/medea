use actix::Actor;
use failure::Error;
use futures::future::Future;
use hashbrown::HashMap;
use medea::{
    api::{client::server::Server, control::grpc},
    conf::Conf,
    log::{self, prelude::*},
    shutdown::{self, GracefulShutdown},
    signalling::{
        room_repo::RoomRepository,
        room_service::{RoomService, StartStaticRooms},
    },
    turn::new_turn_auth_service,
    AppContext,
};

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

                room_service
                    .clone()
                    .send(StartStaticRooms)
                    .map_err(|e| {
                        error!("StartStaticRooms mailbox error: {:?}", e)
                    })
                    .map(|result| {
                        if let Err(e) = result {
                            panic!("{}", e);
                        }
                    })
                    .map(move |_| {
                        let grpc_addr =
                            grpc::server::run(room_service, app_context);
                        graceful_shutdown.do_send(shutdown::Subscribe(
                            shutdown::Subscriber {
                                priority: shutdown::Priority(1),
                                addr: grpc_addr.clone().recipient(),
                            },
                        ));

                        let server = Server::run(room_repo, config).unwrap();
                        graceful_shutdown.do_send(shutdown::Subscribe(
                            shutdown::Subscriber {
                                priority: shutdown::Priority(1),
                                addr: server.recipient(),
                            },
                        ));
                    })
            })
    })
    .unwrap();

    Ok(())
}
