//! Medea media server application.

#![forbid(non_ascii_idents, unsafe_code)]

use actix::{Actor, System};
use failure::Error;
use medea::{
    api::{client::server::Server, control::grpc},
    conf::Conf,
    log::{self, prelude::*},
    shutdown::{self, GracefulShutdown},
    signalling::{RoomRepository, RoomService},
    turn::new_turn_auth_service,
    AppContext,
};

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    let config = Conf::parse()?;

    if let Some(lvl) = config.log.level() {
        std::env::set_var("RUST_LOG", lvl.as_str());
    }

    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init()?;

    info!("{:?}", config);

    let sys = System::new();
    sys.block_on(async move {
        let turn_service = new_turn_auth_service(&config.ice)?;
        let graceful_shutdown =
            GracefulShutdown::new(config.shutdown.timeout).start();
        let app_context = AppContext::new(config.clone(), turn_service);

        let room_repo = RoomRepository::new();
        let room_service = RoomService::new(
            room_repo.clone(),
            app_context.clone(),
            graceful_shutdown.clone(),
        )?
        .start();

        medea::api::control::start_static_rooms(&room_service).await?;

        let (grpc_server_addr, grpc_server_fut) =
            grpc::server::run(room_service, &app_context);
        let server = Server::run(room_repo, config)?;

        shutdown::subscribe(
            &graceful_shutdown,
            grpc_server_addr.recipient(),
            shutdown::Priority(1),
        );

        shutdown::subscribe(
            &graceful_shutdown,
            server.recipient(),
            shutdown::Priority(1),
        );

        grpc_server_fut.await??;
        info!("Started medea system");

        Ok(())
    })
    .map_err(|err: Error| err)?;
    sys.run().map_err(Into::into)
}
