//! [TURN] server managing implementation.
//!
//! [TURN]: https://webrtcglossary.com/turn

pub mod coturn_stats;
mod cli;
pub mod repo;
pub mod service;

#[doc(inline)]
pub use self::service::{
    new_turn_auth_service, TurnAuthService, TurnServiceErr, UnreachablePolicy,
};

#[cfg(test)]
pub use self::service::test::new_turn_auth_service_mock;
