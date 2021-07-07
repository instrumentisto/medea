//! [TURN] server managing implementation.
//!
//! [TURN]: https://webrtcglossary.com/turn

mod coturn;
mod ice_user;
mod static_service;

use std::sync::Arc;

use async_trait::async_trait;
use core::fmt;
use derive_more::{Display, From};
use failure::Fail;
use medea_client_api_proto::{PeerId, RoomId};

use crate::{conf, turn::static_service::StaticService};

use self::coturn::{CoturnCliError, Service as CoturnService, TurnDatabaseErr};

#[doc(inline)]
pub use self::ice_user::{EmptyIceServersListErr, IceUser, IceUsers};

#[cfg(test)]
pub use self::test::new_turn_auth_service_mock;

/// Error which can happen in [`TurnAuthService`].
#[derive(Display, Debug, Fail, From)]
pub enum TurnServiceErr {
    #[display(fmt = "Error accessing TurnAuthRepo: {}", _0)]
    TurnAuthRepoErr(TurnDatabaseErr),

    #[display(fmt = "Error operating CoturnTelnetClient: {}", _0)]
    CoturnCliErr(CoturnCliError),

    #[display(fmt = "Timeout exceeded while trying to insert/delete IceUser")]
    #[from(ignore)]
    TimedOut,
}

/// Defines [`TurnAuthService`] behaviour if remote database is unreachable
#[derive(Debug)]
pub enum UnreachablePolicy {
    /// Error will be propagated if request to db fails cause it is
    /// unreachable.
    ReturnErr,

    /// Static member credentials will be returned if request to db fails cause
    /// it is unreachable.
    ReturnStatic,
}

/// Manages Turn server credentials.
#[async_trait]
pub trait TurnAuthService: fmt::Debug + Send + Sync {
    /// Generates and registers Turn credentials.
    async fn create(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        policy: UnreachablePolicy,
    ) -> Result<Vec<IceUser>, TurnServiceErr>;
}

/// Create new instance [`TurnAuthService`].
///
/// # Errors
///
/// Errors with [`TurnServiceErr::TurnAuthRepoErr`] if authentication in Redis
/// fails.
pub fn new_turn_auth_service<'a>(
    cf: &conf::Turn,
) -> Result<Arc<dyn TurnAuthService + 'a>, TurnServiceErr> {
    match cf {
        conf::Turn::Static { r#static } => {
            Ok(Arc::new(StaticService::new(r#static.servers.clone())))
        }
        conf::Turn::Coturn { coturn } => {
            Ok(Arc::new(CoturnService::new(coturn)?))
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use crate::turn::IceUser;

    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct TurnAuthServiceMock;

    #[async_trait]
    impl TurnAuthService for TurnAuthServiceMock {
        async fn create(
            &self,
            _: RoomId,
            _: PeerId,
            _: UnreachablePolicy,
        ) -> Result<Vec<IceUser>, TurnServiceErr> {
            Ok(vec![IceUser::new_coturn_static(
                "5.5.5.5:1234".parse().unwrap(),
                "username".into(),
                "password".into(),
            )])
        }
    }

    pub fn new_turn_auth_service_mock() -> Arc<dyn TurnAuthService> {
        Arc::new(TurnAuthServiceMock)
    }
}
