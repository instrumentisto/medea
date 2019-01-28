use crate::{
    api::control::{Member, MemberRepository},
    log::prelude::*,
};

#[macro_use]
mod utils;

mod api;
mod errors;
mod log;

fn main() {
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);

    let members = hashmap! {
        1 => Member{id: 1, credentials: "caller_credentials".to_owned()},
        2 => Member{id: 2, credentials: "responder_credentials".to_owned()},
    };

    let repo = MemberRepository { members };
    if let Ok(member) = repo.get_member(1) {
        info!("{:?}", member);
        info!("Hooray!");
        warn!("It works");
    }
}
