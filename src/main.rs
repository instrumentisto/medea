use slog::{o, slog_debug, slog_error, slog_info, slog_trace, slog_warn};
use slog_scope::{debug, error, info, trace, warn};

mod log;

fn main() {
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);

    info!("Exit");
}
