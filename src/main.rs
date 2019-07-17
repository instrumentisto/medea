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

use std::io;

use actix::prelude::*;
use dotenv::dotenv;
use futures::IntoFuture as _;
use log::prelude::*;

use crate::{
    api::{client::server, control::Member},
    conf::Conf,
    media::create_peers,
    shutdown::GracefulShutdown,
    signalling::{Room, RoomsRepository},
    turn::new_turn_auth_service,
};

fn main() -> io::Result<()> {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    actix::run(move || {
        futures::future::lazy(Conf::parse)
            .map_err(|err| error!("Error parsing config {:?}", err))
            .and_then(|config| {
                info!("{:?}", config);

                new_turn_auth_service(&config.turn)
                    .map_err(|err| {
                        error!("Error creating TurnAuthService {:?}", err)
                    })
                    .join(Ok(config))
            })
            .and_then(|(turn_auth_service, config)| {
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

                let graceful_shutdown_addr =
                    GracefulShutdown::new(config.shutdown.timeout).start();
                graceful_shutdown_addr.do_send(shutdown::Subscribe(
                    shutdown::Subscriber {
                        addr: room.clone().recipient(),
                        priority: shutdown::Priority(1),
                    },
                ));

                let rooms = hashmap! {1 => room};
                let rooms_repo = RoomsRepository::new(rooms);

                server::run(rooms_repo, config)
                    .map_err(|err| {
                        error!("Error starting application {:?}", err)
                    })
                    .map(|server_addr| {
                        graceful_shutdown_addr.do_send(shutdown::Subscribe(
                            shutdown::Subscriber {
                                addr: server_addr.recipient(),
                                priority: shutdown::Priority(5),
                            },
                        ));
                    })
                    .into_future()
            })
    })
}
