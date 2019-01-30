use actix::prelude::*;
use dotenv::dotenv;

use crate::api::client::*;

#[macro_use]
mod utils;

mod api;
mod errors;
mod log;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    let _guard = slog_stdlog::init().unwrap();

    let sys = System::new("medea");
    server::run();
    let _ = sys.run();
}
