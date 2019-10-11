//! REST mock server for gRPC Medea's Control API.

pub mod client;
pub mod prelude;
pub mod server;

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
                .help("Address where host control-api-mock-server.")
                .default_value("0.0.0.0:8000")
                .long("addr")
                .short("a"),
        )
        .arg(
            Arg::with_name("medea_addr")
                .help("Address to medea's gRPC control API.")
                .default_value("0.0.0.0:6565")
                .long("medea-addr")
                .short("m"),
        )
        .get_matches();

    let _log_guard = init_logger();

    let sys = actix::System::new("control-api-mock");
    server::run(&opts);
    sys.run().unwrap();
}

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
