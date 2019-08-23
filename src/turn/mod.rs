//! [TURN] server managing implementation.
//!
//! [TURN]: https://webrtcglossary.com/turn

pub mod repo;
pub mod service;

#[doc(inline)]
pub use self::service::{
    new_turn_auth_service, BoxedTurnAuthService, TurnAuthService,
    TurnServiceErr, UnreachablePolicy,
};

#[cfg(test)]
pub use self::service::test::new_turn_auth_service_mock;
