//! [TURN] server managing implementation.
//!
//! [TURN]: https://webrtcglossary.com/turn

pub mod allocation_event;
pub mod cli;
pub mod coturn_metrics;
pub mod ice_user;
pub mod repo;
pub mod service;

use std::fmt::Debug;

use derive_more::Display;
use futures::future::BoxFuture;
use medea_client_api_proto::{PeerId, RoomId};

use crate::turn::{
    cli::CoturnCliError, ice_user::IcePassword, repo::TurnDatabaseErr,
};

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

/// Medea's [Coturn] realm name.
const COTURN_REALM: &str = "medea";

/// Username of Coturn user.
#[derive(Clone, Debug, Display, Eq, Hash, PartialEq)]
#[display(fmt = "{}_{}", room_id, peer_id)]
pub struct CoturnUsername {
    /// [`RoomId`] of the Room this Coturn user is created for.
    pub room_id: RoomId,

    /// [`PeerId`] of the Peer this Coturn user is created for.
    pub peer_id: PeerId,
}

/// Abstraction over remote database used to store Turn server
/// credentials.
#[cfg_attr(test, mockall::automock)]
pub trait TurnDatabase: Send + Debug {
    /// Inserts provided [`IceUsername`] and [`IcePassword`] into remote Redis
    /// database.
    ///
    /// # Errors
    ///
    /// Errors if unable to establish connection with database, or database
    /// request fails.
    fn insert(
        &self,
        username: &'_ IceUsername,
        pass: &'_ IcePassword,
    ) -> BoxFuture<'static, Result<(), TurnDatabaseErr>>;

    /// Deletes provided [`IceUsername`].
    ///
    /// # Errors
    ///
    /// Errors if unable to establish connection with database, or database
    /// request fails.
    fn remove(
        &self,
        username: &'_ IceUsername,
    ) -> BoxFuture<'static, Result<(), TurnDatabaseErr>>;
}

#[cfg(test)]
impl_debug_by_struct_name!(MockTurnDatabase);

/// Abstraction over object which manages [Coturn] server sessions.
#[cfg_attr(test, mockall::automock)]
pub trait TurnSessionManager: Send + Debug {
    /// Forcibly closes provided [`IceUsername`]'s sessions on [Coturn] server.
    ///
    /// # Errors
    ///
    /// When:
    /// - establishing connection with [Coturn] fails;
    /// - retrieving `user`' sessions from [Coturn] fails;
    /// - deleting retrieved `user`' sessions fails.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    ///
    /// # Errors
    ///
    /// With [`CoturnCliError::PoolError`] if could not get or establish new
    /// connection in pool.
    ///
    /// With [`CoturnCliError::CliError`] in case of unexpected protocol error.
    fn delete_session(
        &self,
        user: &'_ IceUsername,
    ) -> BoxFuture<'static, Result<(), CoturnCliError>>;
}

#[cfg(test)]
impl_debug_by_struct_name!(MockTurnSessionManager);
