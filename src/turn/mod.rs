//! [TURN] server managing implementation.
//!
//! [TURN]: https://webrtcglossary.com/turn

pub mod cli;
pub mod coturn_stats;
pub mod repo;
pub mod service;
pub mod stats_validator;

#[doc(inline)]
pub use self::service::{
    new_turn_auth_service, TurnAuthService, TurnServiceErr, UnreachablePolicy,
};

#[cfg(test)]
pub use self::service::test::new_turn_auth_service_mock;
