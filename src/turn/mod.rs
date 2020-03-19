//! [TURN] server managing implementation.
//!
//! [TURN]: https://webrtcglossary.com/turn

pub mod allocation_event;
pub mod cli;
pub mod coturn_metrics;
pub mod repo;
pub mod service;

use derive_more::Display;
use medea_client_api_proto::PeerId;

use crate::api::control::RoomId;

#[doc(inline)]
pub use self::service::{
    new_turn_auth_service, TurnAuthService, TurnServiceErr, UnreachablePolicy,
};

#[cfg(test)]
pub use self::service::test::new_turn_auth_service_mock;

/// Username of the Coturn user.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Display)]
#[display(fmt = "{}_{}", room_id, peer_id)]
pub struct CoturnUsername {
    /// [`RoomId`] of [`Room`] for which this Coturn user is created.
    pub room_id: RoomId,

    /// [`PeerId`] of [`PeerConnection`] for which this Coturn user is created.
    pub peer_id: PeerId,
}
