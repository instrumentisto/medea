use std::sync::Arc;

use failure::Error;
use futures::future::Future;
use medea::{
    api::client::server,
    conf::Conf,
    log::{self, prelude::*},
    signalling::room_repo::RoomsRepository,
    start_static_rooms, App,
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
            .and_then(|res| {
                let (rooms, turn_service) = res.unwrap();
                let app = Arc::new(App {
                    config: config.clone(),
                    turn_service,
                });

                info!(
                    "Loaded rooms: {:?}",
                    rooms.iter().map(|(id, _)| &id.0).collect::<Vec<&String>>()
                );
                let room_repo = RoomsRepository::new(rooms, app);
                server::run(room_repo, config)
                    .map_err(|e| error!("Server {:?}", e))
            })
    })
    .unwrap();

    Ok(())
}
