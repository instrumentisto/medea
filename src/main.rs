use actix::{System, Arbiter, Actor as _};
use failure::Error;
use medea::{
    api::client::server,
    conf::Conf,
    log::{self, prelude::*},
    signalling::room_repo::RoomsRepository,
    start_static_rooms,
};

use medea::api::grpc;

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init()?;

    let sys = System::new("medea");

    let config = Conf::parse()?;
    info!("{:?}", config);

    let rooms = start_static_rooms(&config)?;
    info!(
        "Loaded rooms: {:?}",
        rooms.iter().map(|(id, _)| &id.0).collect::<Vec<&String>>()
    );
    let room_repo = RoomsRepository::new(rooms);
    server::run(room_repo.clone(), config.clone());
    let room_repo_addr = RoomsRepository::start_in_arbiter(&Arbiter::new(), move |_| {
        room_repo
    });
    let _addr = grpc::server::run(room_repo_addr, config);

    let _ = sys.run();

    Ok(())
}
