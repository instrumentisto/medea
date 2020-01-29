//! Medea media server application.

use std::collections::HashMap;

use actix::{Actor, Arbiter, System};
use failure::Error;
use futures::FutureExt as _;
use medea::{
    api::{client::server::Server, control::grpc},
    conf::Conf,
    log::{self, prelude::*},
    shutdown::{self, GracefulShutdown},
    signalling::{room_repo::RoomRepository, room_service::RoomService},
    turn::new_turn_auth_service,
    AppContext,
};

/// Runs [`parking_log`] deadlock detector.
///
/// When feature `deadlock_detection` is enabled, deadlocks of
/// [`parking_lot::Mutex`], [`parking_lot::RwLock`],
/// [`parking_lot::ReentrantMutex`] will be printed into logs.
///
/// This is _experimental_ feature and disable by default.
#[cfg(feature = "deadlock_detection")]
fn run_deadlock_detector() {
    use std::{thread, time::Duration};

    use parking_lot::deadlock;

    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(10));
        let deadlocks = deadlock::check_deadlock();
        if deadlocks.is_empty() {
            continue;
        }

        deadlocks
            .iter()
            .enumerate()
            .flat_map(|(i, threads)| {
                threads.iter().map(move |thread| (i, thread))
            })
            .for_each(|(i, t)| {
                println!(
                    "Deadlock #{}\nThread ID {:#?}\n{:#?}",
                    i,
                    t.thread_id(),
                    t.backtrace()
                )
            });
    });
}

fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    let config = Conf::parse()?;

    if let Some(lvl) = config.log.level() {
        std::env::set_var("RUST_LOG", lvl.as_str());
    }

    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init()?;

    #[cfg(feature = "deadlock_detection")]
    run_deadlock_detector();

    info!("{:?}", config);

    let sys = System::new("medea");
    Arbiter::spawn(
        async move {
            let turn_service = new_turn_auth_service(&config.turn)?;
            let graceful_shutdown =
                GracefulShutdown::new(config.shutdown.timeout).start();
            let app_context = AppContext::new(config.clone(), turn_service);

            let room_repo = RoomRepository::new(HashMap::new());
            let room_service = RoomService::new(
                room_repo.clone(),
                app_context.clone(),
                graceful_shutdown.clone(),
            )
            .start();

            medea::api::control::start_static_rooms(&room_service).await?;

            let grpc_server =
                grpc::server::run(room_service, &app_context).await;
            let server = Server::run(room_repo, config)?;

            shutdown::subscribe(
                &graceful_shutdown,
                grpc_server.recipient(),
                shutdown::Priority(1),
            );

            shutdown::subscribe(
                &graceful_shutdown,
                server.recipient(),
                shutdown::Priority(1),
            );
            Ok(())
        }
        .map(|res: Result<(), Error>| match res {
            Ok(_) => info!("Started system"),
            Err(e) => error!("Startup error: {:?}", e),
        }),
    );
    sys.run().map_err(Into::into)
}
