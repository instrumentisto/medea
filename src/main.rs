use actix::System;
use medea::{
    api::client::server,
    conf::Conf,
    log::{self, prelude::*},
    signalling::room_repo::RoomsRepository,
    start_static_rooms,
};

fn main() {
    dotenv::dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    let sys = System::new("medea");

    let config = Conf::parse().unwrap();
    info!("{:?}", config);

    match start_static_rooms(&config) {
        Ok(r) => {
            let room_repo = RoomsRepository::new(r);
            server::run(room_repo, config);
        }
        Err(e) => {
            error!("Server not started because of error: '{}'", e);
            System::current().stop_with_code(100);
        }
    };

    let _ = sys.run();
}
