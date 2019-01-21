use actix::prelude::*;
use im::hashmap::HashMap;

use crate::{
    api::control::{Member, MemberRepository},
    log::prelude::*,
};

mod api;
mod errors;
mod log;

fn main() {
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);

    let sys = actix::System::new("medea");
    let addr = Arbiter::start(move |_| MemberRepository::default());
    let _ = sys.run();

    info!("Hooray!");
    warn!("It works");
}
