//! Medea media server application.

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

/// Global app context.
#[derive(Debug, Clone)]
pub struct AppContext {
    pub config: Arc<Conf>,
    pub turn_service: Arc<dyn TurnAuthService>,
}

impl AppContext {
    pub fn new(config: Conf, turn: Arc<dyn TurnAuthService>) -> Self {
        Self {
            config: Arc::new(config),
            turn_service: turn,
        }
    }
}
