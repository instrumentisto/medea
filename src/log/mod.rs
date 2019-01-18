use std::io;

use chrono::Local;
use slog::{o, Drain, Duplicate, FnValue, Fuse, Level, Logger, PushFnValue, Record};
use slog_async::Async;
use slog_json::Json;

/// Builds logger which prints all its logs to `w_out`,
/// but WARN level (and higher) logs to second writer.
/// All logs are written in JSON format with key-value pairs
/// such as level and timestamp.
pub fn new_dual_logger<W1, W2>(w_out: W1, w_err: W2) -> Logger
where
    W1: io::Write + Send + 'static,
    W2: io::Write + Send + 'static,
{
    let drain_out = Json::new(w_out).build();
    let drain_err = Json::new(w_err).build();
    let drain = Duplicate(
        drain_out.filter(|r| !r.level().is_at_least(Level::Warning)),
        drain_err.filter_level(Level::Warning),
    )
    .map(Fuse);
    let drain = Async::new(drain).build().fuse();
    add_default_keys(Logger::root(drain, o!()))
}

/// Build logger which writes all its logs to writer.
/// All logs are written in JSON format with key-value pairs
/// such as level and timestamp.
pub fn new_logger<W>(w: W) -> Logger
where
    W: io::Write + Send + 'static,
{
    let drain = Json::new(w).build().fuse();
    let drain = Async::new(drain).build().fuse();
    add_default_keys(Logger::root(drain, o!()))
}

/// Add default key-values for log:
///
/// * `time` - timestamp
/// * `lvl` - record logging level name
/// * `msg` - msg - formatted logging message
fn add_default_keys(logger: Logger) -> Logger {
    logger.new(o!(
        "msg" => PushFnValue(move |record : &Record, ser| {
            ser.emit(record.msg())
        }),
        "time" => PushFnValue(move |_ : &Record, ser| {
            ser.emit(Local::now().to_rfc3339())
        }),
        "lvl" => FnValue(move |rinfo : &Record| {
            rinfo.level().as_str()
        }),
    ))
}
