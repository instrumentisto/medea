//! Medea media server.

#![allow(clippy::module_name_repetitions)]
#![deny(broken_intra_doc_links)]

#[macro_use]
mod utils;
mod api;
mod conf;
mod log;
mod media;
mod shutdown;
mod signalling;
mod turn;

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
