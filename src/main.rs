pub use actix::prelude::*;
use dotenv::dotenv;

pub use slog::{o, slog_debug, slog_error, slog_info, slog_trace, slog_warn};
pub use slog_scope::{debug, error, info, trace, warn};

pub mod api;
mod errors;
mod log;
mod server;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    let _guard = slog_stdlog::init().unwrap();

    let sys = actix::System::new("medea");
    init_repo();
    server::run();
    let _ = sys.run();

    info!("Exit");
}
