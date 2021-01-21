//! Implementation of managing [Coturn] [TURN] server.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [TURN]: https://webrtcglossary.com/turn/

use std::{fmt, slice, sync::Arc};

use async_trait::async_trait;
use derive_more::{Display, From};
use failure::Fail;
use futures::{
    channel::mpsc,
    future::{self, AbortHandle},
    StreamExt as _,
};
use medea_client_api_proto::{PeerId, RoomId};
use redis::ConnectionInfo;

use crate::{
    conf,
    log::prelude as log,
    turn::{
        cli::{CoturnCliError, CoturnTelnetClient},
        ice_user::{IcePassword, IceUsername},
        repo::{TurnDatabase, TurnDatabaseErr},
    },
    utils::MpscOneshotSender,
};

use super::IceUser;

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
    ) -> Result<IceUser, TurnServiceErr>;
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

    /// Channel sender signalling about an [`IseUser`] no longer being used and
    /// that it should be removed from [Coturn].
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    drop_tx: MpscOneshotSender<IceUsername>,

    // TODO: tokio 1.0 has abort() function in JoinHandle,
    //       so we can use it directly.
    /// [`AbortHandle`] to task that cleanups [`IceUser`]s.
    users_cleanup_task: AbortHandle,
}

impl Service {
    /// Returns [`IceUser`] with static credentials.
    fn static_user(&self) -> IceUser {
        IceUser::new_static(
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
    ///
    /// [`MemberId`]: medea_client_api_proto::MemberId
    async fn create(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        policy: UnreachablePolicy,
    ) -> Result<IceUser, TurnServiceErr> {
        let ice_user = IceUser::new_non_static(
            self.turn_address.clone(),
            &room_id,
            peer_id,
            IcePassword::generate(),
            self.drop_tx.clone(),
        );

        match self.turn_db.insert(&ice_user).await {
            Ok(_) => Ok(ice_user),
            Err(err) => match policy {
                UnreachablePolicy::ReturnErr => Err(err.into()),
                UnreachablePolicy::ReturnStatic => Ok(self.static_user()),
            },
        }
    }
}

impl Drop for Service {
    #[inline]
    fn drop(&mut self) {
        self.users_cleanup_task.abort();
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
        ConnectionInfo::from(&cf.db.redis),
    )?;

    let coturn_cli = CoturnTelnetClient::new(
        (cf.cli.host.clone(), cf.cli.port),
        cf.cli.pass.to_string(),
        cf.cli.pool.into(),
    );

    let (tx, mut rx) = mpsc::unbounded();

    let users_cleanup_task = {
        let db = turn_db.clone();
        let cli = coturn_cli.clone();
        let (fut, handle) = future::abortable(async move {
            while let Some(user) = rx.next().await {
                let users = slice::from_ref(&user);
                if let Err(e) = db.remove(users).await {
                    log::warn!(
                        "Failed to remove IceUser(name: {}) from Redis: {}",
                        user,
                        e,
                    );
                }
                if let Err(e) = cli.delete_sessions(users).await {
                    log::warn!(
                        "Failed to remove IceUser(name: {}) from Coturn: {}",
                        user,
                        e,
                    );
                }
            }
        });
        tokio::spawn(fut);
        handle
    };

    let turn_service = Service {
        turn_db,
        coturn_cli,
        db_pass: cf.db.redis.pass.to_string(),
        turn_address: cf.addr(),
        turn_username: cf.user.to_string(),
        turn_password: cf.pass.to_string(),
        drop_tx: MpscOneshotSender::from(tx),
        users_cleanup_task,
    };

    Ok(Arc::new(turn_service))
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
        ) -> Result<IceUser, TurnServiceErr> {
            Ok(IceUser::new_static(
                "5.5.5.5:1234".parse().unwrap(),
                "username".into(),
                "password".into(),
            ))
        }
    }

    pub fn new_turn_auth_service_mock() -> Arc<dyn TurnAuthService> {
        Arc::new(TurnAuthServiceMock)
    }
}
