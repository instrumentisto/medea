//! [TURN] server managing implementation.
//!
//! [TURN]: https://webrtcglossary.com/turn

pub mod allocation_event;
pub mod cli;
pub mod coturn_metrics;
pub mod ice_user;
pub mod repo;
pub mod service;

use derive_more::Display;
use medea_client_api_proto::{PeerId, RoomId};

#[doc(inline)]
pub use self::{
    ice_user::{IceUser, IceUsername},
    service::{
        new_turn_auth_service, TurnAuthService, TurnServiceErr,
        UnreachablePolicy,
    },
};

#[cfg(test)]
pub use self::service::test::new_turn_auth_service_mock;

/// Username of Coturn user.
#[derive(Clone, Debug, Display, Eq, Hash, PartialEq)]
#[display(fmt = "{}_{}", room_id, peer_id)]
pub struct CoturnUsername {
    /// [`RoomId`] of the Room this Coturn user is created for.
    pub room_id: RoomId,

    /// [`PeerId`] of the Peer this Coturn user is created for.
    pub peer_id: PeerId,
}
