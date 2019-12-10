//! Medea media server.

#![allow(clippy::module_name_repetitions)]

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

use crate::{conf::Conf, turn::TurnAuthService};

/// Global application context.
#[derive(Clone, Debug)]
pub struct AppContext {
    /// [Medea] configuration.
    ///
    /// [Medea]: https://github.com/instrumentisto/medea
    pub config: Arc<Conf>,

    /// Reference to [`TurnAuthService`].
    pub turn_service: Arc<dyn TurnAuthService>,
}

impl AppContext {
    /// Creates new [`AppContext`].
    #[inline]
    pub fn new(config: Conf, turn: Arc<dyn TurnAuthService>) -> Self {
        Self {
            config: Arc::new(config),
            turn_service: turn,
        }
    }
}
