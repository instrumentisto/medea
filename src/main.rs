use actix::prelude::*;

use crate::{
    api::control::{Member, MemberRepository},
    log::prelude::*,
    utils::hashmap,
};

mod api;
mod errors;
mod log;
mod utils;

fn main() {
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);

    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };

    let sys = actix::System::new("medea");
    let _addr = Arbiter::start(move |_| {
        info!("Hooray!");
        warn!("It works");
        MemberRepository { members }
    });
    let _ = sys.run();
}
