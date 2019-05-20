//! Medea media server application.

#[macro_use]
pub mod utils;
pub mod api;
pub mod conf;
pub mod log;
pub mod media;
pub mod signalling;

use actix::prelude::*;
use dotenv::dotenv;
use log::prelude::*;
use std::sync::Arc;

use crate::{
    api::{
        client::server,
        control::Member,
        control::{
            control_room_repo::ControlRoomRepository, load_from_file,
            ControlRoom, RoomRequest,
        },
    },
    conf::Conf,
    media::create_peers,
    signalling::Room,
};

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let config = Conf::parse().unwrap();
    info!("{:?}", config);

    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };
    let peers = create_peers(1, 2);
    let (id, room_spec) = match load_from_file("room_spec.yml").unwrap() {
        RoomRequest::Room { id, spec } => (id, spec),
    };
    let client_room =
        Room::new(id, members, peers, config.rpc.reconnect_timeout);
    let client_room = Arbiter::start(move |_| client_room);

    let control_room = ControlRoom {
        client_room,
        spec: room_spec,
    };
    let rooms = hashmap! {id => control_room};

    let room_repo = Arc::new(ControlRoomRepository::new(rooms));

    server::run(Arc::clone(&room_repo), config);
    let _ = sys.run();
}
