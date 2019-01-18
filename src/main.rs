use crate::log::prelude::*;

mod log;

fn main() {
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);

    info!("Hooray!");
    warn!("It works");
}
