//! Implementation of managing [Coturn] [TURN] server.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [TURN]: https://webrtcglossary.com/turn/

use std::{fmt, sync::Arc};

use async_trait::async_trait;
use derive_more::{Display, From};
use failure::Fail;
use rand::{distributions::Alphanumeric, Rng};
use redis::ConnectionInfo;

use crate::{
    api::control::{MemberId, RoomId},
    conf,
    media::IceUser,
    turn::{
        cli::{CoturnCliError, CoturnTelnetClient},
        repo::{TurnDatabase, TurnDatabaseErr},
    },
};

static TURN_PASS_LEN: usize = 16;

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
        member_id: MemberId,
        room_id: RoomId,
        policy: UnreachablePolicy,
    ) -> Result<IceUser, TurnServiceErr>;

    /// Deletes batch of [`IceUser`]s.
    async fn delete(&self, users: &[IceUser]) -> Result<(), TurnServiceErr>;
}

/// [`TurnAuthService`] implementation backed by Redis database.
#[derive(Debug)]
struct Service {
    /// Turn credentials repository.
    turn_db: TurnDatabase,

    /// Client of [Coturn] server admin interface.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    coturn_cli: CoturnTelnetClient,

    /// TurnAuthRepo password.
    db_pass: String,

    /// Turn server address.
    turn_address: String,

    /// Turn server static user.
    turn_username: String,

    /// Turn server static user password.
    turn_password: String,
}

impl Service {
    /// Generates random alphanumeric string of specified length.
    fn generate_pass(n: usize) -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(n)
            .collect()
    }

    /// Returns [`IceUser`] with static credentials.
    fn static_user(&self) -> IceUser {
        IceUser::new(
            self.turn_address.clone(),
            self.turn_username.clone(),
            self.turn_password.clone(),
        )
    }
}

#[async_trait]
impl TurnAuthService for Service {
    /// Generates [`IceUser`] with saved Turn address, provided [`MemberId`] and
    /// random password. Inserts created [`IceUser`] into [`TurnDatabase`].
    async fn create(
        &self,
        member_id: MemberId,
        room_id: RoomId,
        policy: UnreachablePolicy,
    ) -> Result<IceUser, TurnServiceErr> {
        let ice_user = IceUser::build(
            self.turn_address.clone(),
            &room_id,
            &member_id.0,
            Self::generate_pass(TURN_PASS_LEN),
        );

        match self.turn_db.insert(&ice_user).await {
            Ok(_) => Ok(ice_user),
            Err(err) => match policy {
                UnreachablePolicy::ReturnErr => Err(err.into()),
                UnreachablePolicy::ReturnStatic => Ok(self.static_user()),
            },
        }
    }

    /// Deletes provided [`IceUser`]s from [`TurnDatabase`] and closes their
    /// sessions on [Coturn] server.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    async fn delete(&self, users: &[IceUser]) -> Result<(), TurnServiceErr> {
        if users.is_empty() {
            return Ok(());
        }

        // leave only non static users
        let users = users.iter().filter(|u| !u.is_static()).collect::<Vec<_>>();
        self.turn_db.remove(users.as_slice()).await?;
        self.coturn_cli.delete_sessions(users.as_slice()).await?;
        Ok(())
    }
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
    let turn_db = TurnDatabase::new(
        cf.db.redis.connect_timeout,
        ConnectionInfo {
            addr: Box::new(redis::ConnectionAddr::Tcp(
                cf.db.redis.host.to_string(),
                cf.db.redis.port,
            )),
            db: cf.db.redis.db_number,
            passwd: if cf.db.redis.pass.is_empty() {
                None
            } else {
                Some(cf.db.redis.pass.to_string())
            },
        },
    )?;

    let coturn_cli = CoturnTelnetClient::new(
        (cf.cli.host.clone(), cf.cli.port),
        cf.cli.pass.to_string(),
        cf.cli.pool.into(),
    );

    let turn_service = Service {
        turn_db,
        coturn_cli,
        db_pass: cf.db.redis.pass.to_string(),
        turn_address: cf.addr(),
        turn_username: cf.user.to_string(),
        turn_password: cf.pass.to_string(),
    };

    Ok(Arc::new(turn_service))
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use crate::media::IceUser;

    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct TurnAuthServiceMock;

    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[async_trait]
    impl TurnAuthService for TurnAuthServiceMock {
        async fn create(
            &self,
            _: MemberId,
            _: RoomId,
            _: UnreachablePolicy,
        ) -> Result<IceUser, TurnServiceErr> {
            Ok(IceUser::new(
                "5.5.5.5:1234".parse().unwrap(),
                "username".into(),
                "password".into(),
            ))
        }

        async fn delete(&self, _: &[IceUser]) -> Result<(), TurnServiceErr> {
            Ok(())
        }
    }

    pub fn new_turn_auth_service_mock() -> Arc<dyn TurnAuthService> {
        Arc::new(TurnAuthServiceMock)
    }
}
