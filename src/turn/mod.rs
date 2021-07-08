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

/// Errors happening in [`TurnAuthService`].
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

/// [`TurnAuthService`] behavior when remote database is unreachable.
#[derive(Debug)]
pub enum UnreachablePolicy {
    /// Error will be propagated if request to database fails.
    Error,

    /// Static member credentials will be returned if request to database
    /// fails.
    Static,
}

/// Manages [TURN] server credentials.
///
/// [TURN]: https://webrtcglossary.com/turn
#[async_trait]
pub trait TurnAuthService: fmt::Debug + Send + Sync {
    /// Generates and registers [TURN] credentials.
    ///
    /// [TURN]: https://webrtcglossary.com/turn
    async fn create(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        policy: UnreachablePolicy,
    ) -> Result<Vec<IceUser>, TurnServiceErr>;
}

/// Create a new instance of [`TurnAuthService`].
///
/// # Errors
///
/// Errors with [`TurnServiceErr::TurnAuthRepoErr`] if authentication in [Redis]
/// database fails.
///
/// [Redis]: https://redis.io
pub fn new_turn_auth_service<'a>(
    cf: &conf::Ice,
) -> Result<Arc<dyn TurnAuthService + 'a>, TurnServiceErr> {
    Ok(match cf.default {
        conf::ice::Kind::Static => {
            let static_servers = cf.r#static.values().cloned().collect();
            Arc::new(StaticService::new(static_servers))
        }
        conf::ice::Kind::Coturn => Arc::new(CoturnService::new(&cf.coturn)?),
    })
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
