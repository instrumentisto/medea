//! Medea media server application.
use actix::prelude::*;
use dotenv::dotenv;

use crate::api::client::*;
use crate::api::control::*;

#[macro_use]
mod utils;

mod api;
mod log;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    let _guard = slog_stdlog::init().unwrap();

    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };

    let members_repo = MemberRepository::new(members);

    let sys = System::new("medea");
    server::run(members_repo);
    let _ = sys.run();
}
