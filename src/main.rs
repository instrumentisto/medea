//! Medea media server application.

use crate::{
    api::control::{Member, MemberRepository},
    log::prelude::*,
};

#[macro_use]
mod utils;

mod api;
mod log;

fn main() {
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);

    let repo = MemberRepository::new(hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    });
    if let Some(member) = repo.get(1) {
        info!("{:?}", member);
        info!("Hooray!");
        warn!("It works");
    }
}
