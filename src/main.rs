//! Medea media server application.

#[macro_use]
extern crate redis_async;
#[macro_use]
pub mod utils;
pub mod api;
pub mod conf;
pub mod log;
pub mod media;
pub mod signalling;

use actix::prelude::*;
use actix_redis::RedisActor;
use dotenv::dotenv;
use log::prelude::*;

use crate::{
    api::{client::server, control::Member},
    conf::Conf,
    media::create_peers,
    signalling::{AuthService, Room, RoomsRepository},
};

#[cfg(not(test))]
fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let config = Conf::parse().unwrap();

    info!("{:?}", config);

    let redis = RedisActor::start(config.redis.get_addr().to_string());
    let coturn_auth = AuthService::new(&config, redis).start();
    let members = hashmap! {
        1 => Member::new(1, "caller_credentials".to_owned()),
        2 => Member::new(2, "responder_credentials".to_owned()),
    };
    let peers = create_peers(1, 2);
    let room =
        Room::new(1, members, peers, config.rpc.reconnect_timeout, coturn_auth);
    let room = Arbiter::start(move |_| room);
    let rooms = hashmap! {1 => room};
    let rooms_repo = RoomsRepository::new(rooms);

    server::run(rooms_repo, config);
    let _ = sys.run();
}
