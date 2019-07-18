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

use std::{env, io};

use actix::prelude::*;
use dotenv::dotenv;
use futures::IntoFuture as _;
use log::prelude::*;

use crate::{
    api::{client::server::Server, control::Member},
    conf::Conf,
    media::create_peers,
    shutdown::GracefulShutdown,
    signalling::{Room, RoomsRepository},
    turn::new_turn_auth_service,
};

fn main() -> io::Result<()> {
    dotenv().ok();
    let config = Conf::parse().unwrap();

    if let Some(lvl) = config.log.level() {
        env::set_var("RUST_LOG", lvl.as_str());
    }
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    info!("{:?}", config);

    actix::run(|| {
        new_turn_auth_service(&config.turn)
            .map_err(|err| error!("Error creating TurnAuthService {:?}", err))
            .and_then(|turn_auth_service| {
                let members = hashmap! {
                    1 => Member::new(1, "caller_credentials".to_owned()),
                    2 => Member::new(2, "responder_credentials".to_owned()),
                };
                let room = Room::new(
                    1,
                    members,
                    create_peers(1, 2),
                    config.rpc.reconnect_timeout,
                    turn_auth_service,
                )
                    .start();
                Ok((room, config))
            })
            .and_then(|(room, config)| {
                let graceful_shutdown =
                    GracefulShutdown::new(config.shutdown.timeout).start();
                graceful_shutdown
                    .send(shutdown::Subscribe(shutdown::Subscriber {
                        addr: room.clone().recipient(),
                        priority: shutdown::Priority(2),
                    }))
                    .map_err(|e| {
                        error!("Shutdown subscription failed for Room: {}", e)
                    })
                    .map(move |_| (room, graceful_shutdown, config))
            })
            .map(|(room, graceful_shutdown, config)| {
                let rooms = hashmap! {1 => room};
                let rooms_repo = RoomsRepository::new(rooms);
                (rooms_repo, graceful_shutdown, config)
            })
            .and_then(|(rooms_repo, graceful_shutdown, config)| {
                Server::run(rooms_repo, config)
                    .map_err(|e| {
                        error!("Error starting Client API HTTP server {:?}", e)
                    })
                    .map(|server| {
                        graceful_shutdown
                            .send(shutdown::Subscribe(shutdown::Subscriber {
                                addr: server.recipient(),
                                priority: shutdown::Priority(1),
                            }))
                            .map_err(|e| {
                                error!(
                                    "Shutdown subscription failed for Client \
                                     API HTTP server: {}",
                                    e
                                )
                            })
                            .map(|_| ())
                    })
                    .into_future()
                    .flatten()
            })
    })
}
