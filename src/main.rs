use actix::prelude::*;
use dotenv::dotenv;
use im::hashmap::HashMap;

use crate::{
    api::control::{Member, MemberRepository},
    log::prelude::*,
};

#[macro_use]
mod utils;
mod api;
mod errors;
mod log;
mod server;

fn main() {
    dotenv().ok();
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);
    let _guard = slog_stdlog::init().unwrap();

    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };

    let sys = actix::System::new("medea");
    server::run();
    let _addr = Arbiter::start(move |_| MemberRepository { members });
    let _ = sys.run();

    info!("Hooray!");
    warn!("It works");
}
