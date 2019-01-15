use slog::{o, slog_debug, slog_error, slog_info, slog_trace, slog_warn};
use slog_scope::{debug, error, info, trace, warn};

mod log;

fn main() {
    let logger = log::new_dual_logger(std::io::stdout(), std::io::stderr());
    let _scope_guard = slog_scope::set_global_logger(logger);

    error!("log error");

    slog_scope::scope(
        &slog_scope::logger().new(o!("scope-extra-data" => "data")),
        || foo(),
    );

    info!("log info");
    warn!("log warning");
}

fn foo() {
    info!("log info inside foo");

    // scopes can be nested!
    slog_scope::scope(
        &slog_scope::logger().new(o!("even-more-scope-extra-data" => "data2")),
        || bar(),
    );
}

fn bar() {
    info!("log info inside bar");
    debug!("debug");
    trace!("log trace");
}
