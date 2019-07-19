use actix::Actor;
use failure::Error;
use futures::future::{Future, IntoFuture as _};
use medea::{
    api::client::server::{self, Server},
    conf::Conf,
    log::{self, prelude::*},
    shutdown::{self, GracefulShutdown},
    signalling::room_repo::RoomsRepository,
    start_static_rooms,
};

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init()?;

    let config = Conf::parse()?;
    info!("{:?}", config);

    actix::run(|| {
        start_static_rooms(&config)
            .map_err(|e| error!("Turn: {:?}", e))
            .map(|res| {
                let graceful_shutdown =
                    GracefulShutdown::new(config.shutdown.timeout).start();
                (res, graceful_shutdown, config)
            })
            .map(|(res, graceful_shutdown, config)| {
                let rooms = res.unwrap();
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
