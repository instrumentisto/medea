use actix::prelude::*;
use im::hashmap::HashMap;

use slog::{o, slog_debug, slog_error, slog_info, slog_trace, slog_warn};
use slog_scope::{debug, error, info, trace, warn};

mod api;
mod errors;
mod log;

use crate::api::control::member::{Member, MemberRepository};

fn main() {
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);

    let sys = actix::System::new("medea");
    run();
    let _ = sys.run();

    info!("Exit");
}

fn run() {
    let mut members = HashMap::new();
    members.insert(
        1,
        Member {
            id: 1,
            credentials: "user1_credentials".to_owned(),
        },
    );
    members.insert(
        2,
        Member {
            id: 2,
            credentials: "user2_credentials".to_owned(),
        },
    );

    let addr = Arbiter::builder().start(move |_| MemberRepository { members });

    info!("Repository created");
}
