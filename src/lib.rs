//! Medea media server.

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
    api::control::callback::repo::CallbackRepository, conf::Conf,
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

    /// Repository which stores all callback services.
    pub callbacks: CallbackRepository,
}

impl AppContext {
    /// Creates new [`AppContext`].
    #[inline]
    pub fn new(config: Conf, turn: Arc<dyn TurnAuthService>) -> Self {
        Self {
            config: Arc::new(config),
            turn_service: turn,
            callbacks: CallbackRepository::new(),
        }
    }
}
