//! Medea media server.

#![allow(clippy::module_name_repetitions)]
#![deny(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
#![forbid(unsafe_code, non_ascii_idents)]

#[macro_use]
pub mod utils;
pub mod api;
pub mod conf;
pub mod log;
pub mod media;
pub mod shutdown;
pub mod signalling;
pub mod turn;

use std::sync::Arc;

use crate::{
    api::control::callback::{
        clients::CallbackClientFactoryImpl, service::CallbackService,
    },
    conf::Conf,
    turn::TurnAuthService,
};

/// Global application context.
#[derive(Clone, Debug)]
pub struct AppContext {
    /// [Medea] configuration.
    ///
    /// [Medea]: https://github.com/instrumentisto/medea
    pub config: Arc<Conf>,

    /// Reference to [`TurnAuthService`].
    pub turn_service: Arc<dyn TurnAuthService>,

    /// Service for sending Control API Callbacks.
    pub callbacks: CallbackService<CallbackClientFactoryImpl>,
}

impl AppContext {
    /// Creates new [`AppContext`].
    #[inline]
    #[must_use]
    pub fn new(config: Conf, turn: Arc<dyn TurnAuthService>) -> Self {
        Self {
            config: Arc::new(config),
            turn_service: turn,
            callbacks: CallbackService::default(),
        }
    }
}
