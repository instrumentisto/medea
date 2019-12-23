//! REST mock server for gRPC [Medea]'s [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]

pub mod api;
pub mod callback;
pub mod client;
pub mod prelude;

use clap::{
    app_from_crate, crate_authors, crate_description, crate_name,
    crate_version, Arg,
};
use slog::{o, Drain};
use slog_scope::GlobalLoggerGuard;

fn main() {
    dotenv::dotenv().ok();

    let opts = app_from_crate!()
        .arg(
            Arg::with_name("addr")
                .help("Address to host medea-control-api-mock-server on.")
                .default_value("0.0.0.0:8000")
                .long("addr")
                .short("a"),
        )
        .arg(
            Arg::with_name("medea_addr")
                .help("Address to Medea's gRPC control API.")
                .default_value("0.0.0.0:6565")
                .long("medea-addr")
                .short("m"),
        )
        .arg(
            Arg::with_name("callback_port")
                .help("Port to listen by gRPC Control API Callback service.")
                .default_value("9099")
                .long("callback-port")
                .short("p"),
        )
        .arg(
            Arg::with_name("callback_host")
                .help("Address to host gRPC Control API Callback service on.")
                .default_value("0.0.0.0")
                .long("callback-host")
                .short("c"),
        )
        .get_matches();

    let _log_guard = init_logger();

    let sys = actix::System::new("control-api-mock");
    let callback_server = callback::server::run(&opts);
    api::run(&opts, callback_server);
    sys.run().unwrap();
}

/// Initializes [`slog`] logger which will output logs with [`slog_term`]'s
/// decorator.
fn init_logger() -> GlobalLoggerGuard {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = slog::Logger::root(drain, o!());
    let scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    scope_guard
}
