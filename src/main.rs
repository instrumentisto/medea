//! Medea media server application.

use actix::prelude::*;
use dotenv::dotenv;

use crate::api::client::*;

mod api;
mod log;
#[macro_use]
mod utils;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    let _guard = slog_stdlog::init().unwrap();

    let sys = System::new("medea");
    server::run();
    let _ = sys.run();
}
