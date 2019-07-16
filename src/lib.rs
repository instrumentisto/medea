//! Medea media server application.

#[macro_use]
pub mod utils;
pub mod api;
pub mod conf;
pub mod log;
pub mod media;
pub mod signalling;
pub mod turn;

use std::sync::Arc;

use crate::{conf::Conf, turn::BoxedTurnAuthService};

/// Global app context.
#[derive(Debug)]
pub struct App {
    pub config: Conf,
    pub turn_service: Arc<BoxedTurnAuthService>,
}
