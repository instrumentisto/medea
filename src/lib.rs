//! Medea media server application.

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

use crate::{conf::Conf, turn::BoxedTurnAuthService};

/// Global app context.
#[derive(Debug, Clone)]
pub struct AppContext {
    pub config: Arc<Conf>,
    pub turn_service: Arc<BoxedTurnAuthService>,
}

impl AppContext {
    pub fn new(config: Conf, turn: BoxedTurnAuthService) -> Self {
        Self {
            config: Arc::new(config),
            turn_service: Arc::new(turn),
        }
    }
}
