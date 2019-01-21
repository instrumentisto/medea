use actix::prelude::*;
use dotenv::dotenv;

use crate::api::control::member::{Member, MemberRepository};
use crate::log::prelude::*;

mod api;
mod errors;
mod log;
mod server;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    let _guard = slog_stdlog::init().unwrap();

    let sys = actix::System::new("medea");
    server::run();
    let addr = Arbiter::start(move |_| MemberRepository::default());
    let _ = sys.run();

    info!("Hooray!");
    warn!("It works");
}
