//! REST mock server for gRPC [Medea]'s [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

// TODO: Remove `clippy::must_use_candidate` once the issue below is resolved:
//       https://github.com/rust-lang/rust-clippy/issues/4779
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]

pub mod api;
pub mod callback;
pub mod client;
pub mod prelude;

use slog::{o, Drain};
use slog_scope::GlobalLoggerGuard;

pub mod proto {
    pub use crate::api::{
        endpoint::{
            AudioSettings, P2pMode, PublishPolicy, VideoSettings,
            WebRtcPlayEndpoint, WebRtcPublishEndpoint,
        },
        member::Member,
        room::Room,
        CreateResponse, Element, ErrorResponse, Response, SingleGetResponse,
    };
}

/// Initializes [`slog`] logger which will output logs with [`slog_term`]'s
/// decorator.
pub fn init_logger() -> GlobalLoggerGuard {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = slog::Logger::root(drain, o!());
    let scope_guard = slog_scope::set_global_logger(logger);
    slog_stdlog::init().unwrap();

    scope_guard
}
