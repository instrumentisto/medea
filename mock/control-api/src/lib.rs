//! REST mock server for gRPC [Medea]'s [Control API].
//!
//! [Medea]: https://github.com/instrumentisto/medea
//! [Control API]: https://tinyurl.com/yxsqplq7

#![allow(clippy::module_name_repetitions)]
#![forbid(non_ascii_idents, unsafe_code)]

pub mod api;
pub mod callback;
pub mod client;
pub mod prelude;

use slog::{o, Drain};
use slog_scope::GlobalLoggerGuard;

pub mod proto {
    pub use crate::api::{
        endpoint::{
            AudioSettings, Endpoint, P2pMode, PublishPolicy, VideoSettings,
            WebRtcPlayEndpoint, WebRtcPublishEndpoint,
        },
        member::{Credentials, Member},
        room::{Room, RoomElement},
        CreateResponse, Element, ErrorResponse, Response, SingleGetResponse,
    };
}

/// Initializes [`slog`] logger outputting logs with a [`slog_term`]'s
/// decorator.
///
/// # Panics
///
/// If [`slog_stdlog`] fails to [initialize](slog_stdlog::init).
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
