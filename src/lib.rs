//! Medea media server.

// TODO: Remove `clippy::must_use_candidate` once the issue below is resolved:
//       https://github.com/rust-lang/rust-clippy/issues/4779
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]
// TODO: REVERT ME BEFORE MERGE!!!
#![allow(clippy::missing_errors_doc)]

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
    api::control::callback::service::CallbackService, conf::Conf,
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

    /// Service for sending [`CallbackEvent`]s.
    ///
    /// [`CallbackEvent`]: crate::api::control::callbacks::CallbackEvent
    pub callbacks: CallbackService,
}

impl AppContext {
    /// Creates new [`AppContext`].
    #[inline]
    pub fn new(config: Conf, turn: Arc<dyn TurnAuthService>) -> Self {
        Self {
            config: Arc::new(config),
            turn_service: turn,
            callbacks: CallbackService::default(),
        }
    }
}
