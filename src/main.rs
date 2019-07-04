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
use dotenv::dotenv;
use log::prelude::*;

use crate::{
    api::{client::server, control::Member},
    conf::Conf,
    media::create_peers,
    signalling::{Room, RoomsRepository},
    turn::new_turn_auth_service,
    utils::graceful_shutdown,
};
use std::panic;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    let _log_guard = slog_stdlog::init().unwrap();

    let config = Conf::parse().unwrap();
    info!("{:?}", config);

    actix::run(|| {
//        futures::future::lazy(|| {
            let members = hashmap! {
            1 => Member::new(1, "caller_credentials".to_owned()),
            2 => Member::new(2, "responder_credentials".to_owned()),
        };

            let peers = create_peers(1, 2);

            graceful_shutdown::create(
                config.system_config.shutdown_timeout,
                System::current(),
            );
            let turn_auth_service = new_turn_auth_service(&config)
                .expect("Unable to start turn service");

            let rpc_reconnect_timeout = config.rpc.reconnect_timeout;

            let room = Room::start_in_arbiter(&Arbiter::new(), move |_| {
                Room::new(
                    1,
                    members,
                    peers,
                    rpc_reconnect_timeout,
                    turn_auth_service,
                )
            });

            // TODO: do subscribe in Room::new, server::run
            graceful_shutdown::subscribe(room.clone().recipient(), 1);

            let rooms = hashmap! {1 => room};
            let rooms_repo = RoomsRepository::new(rooms);

            let http_server = server::run(rooms_repo, config);
            graceful_shutdown::subscribe(http_server.recipient(), 2);

            futures::future::ok(())
    //        })
        })
    .expect("unable to start medea");

}
