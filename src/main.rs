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
use actix::actors::signal;
use dotenv::dotenv;
use log::prelude::*;

use crate::{
    api::{client::server, control::Member},
    conf::Conf,
    media::create_peers,
    signalling::{Room, RoomsRepository},
    turn::new_turn_auth_service,
};
use actix_web::server::StopServer;
use actix::actors::signal::{Subscribe, ProcessSignals};

//struct Signals;
//
//impl Actor for Signals {
//    type Context = Context<Self>;
//}

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();
    let sys = System::new("medea");

    let config = Conf::parse().unwrap();

    info!("{:?}", config);

    let members = hashmap! {
        1 => Member::new(1, "caller_credentials".to_owned()),
        2 => Member::new(2, "responder_credentials".to_owned()),
    };
    let peers = create_peers(1, 2);

    let turn_auth_service =
        new_turn_auth_service(&config).expect("Unable to start turn service");

    let rpc_reconnect_timeout = config.rpc.reconnect_timeout;

    let room = Room::start_in_arbiter(&Arbiter::new(), move |_| {
        Room::new(1, members, peers, rpc_reconnect_timeout, turn_auth_service)
    });

    let rooms = hashmap! {1 => room};
    let rooms_repo = RoomsRepository::new(rooms);

    //me register graceful shutdown signals
//    let my_signal = Signals.start();
//    let process_signals = System::current().registry().get::<signal::ProcessSignals>();
//    process_signals.do_send(Subscribe(my_signal.recipient()));

    server::run(rooms_repo, config);
    let _ = sys.run();
}
