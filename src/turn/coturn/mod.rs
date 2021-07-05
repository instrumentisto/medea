//! Implementation of managing [Coturn] [TURN] server.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [TURN]: https://webrtcglossary.com/turn/

mod allocation_event;
mod cli;
mod coturn_metrics;
mod repo;
mod ice_user;

use std::slice;

use async_trait::async_trait;
use derive_more::Display;
use futures::{channel::mpsc, StreamExt as _};
use medea_client_api_proto::{PeerId, RoomId};
use redis::ConnectionInfo;
use tokio::task::JoinHandle;

use crate::{conf, log::prelude as log, utils::MpscOneshotSender};

use super::{
    TurnAuthService, TurnServiceErr, UnreachablePolicy,
};
use super::IceUser;

use self::{
    ice_user::{IcePassword, IceUsername},
};
use self::{cli::CoturnTelnetClient, repo::TurnDatabase};

pub use self::{cli::CoturnCliError, repo::TurnDatabaseErr};
pub use self::ice_user::CoturnIceUser;

/// Username of Coturn user.
#[derive(Clone, Debug, Display, Eq, Hash, PartialEq)]
#[display(fmt = "{}_{}", room_id, peer_id)]
pub struct CoturnUsername {
    /// [`RoomId`] of the Room this Coturn user is created for.
    pub room_id: RoomId,

    /// [`PeerId`] of the Peer this Coturn user is created for.
    pub peer_id: PeerId,
}

/// [`TurnAuthService`] implementation backed by Redis database.
#[derive(Debug)]
pub struct Service {
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

    /// [`JoinHandle`] to task that cleanups [`IceUser`]s.
    users_cleanup_task: JoinHandle<()>,
}

impl Service {
    /// Create new [`Service`] instance.
    ///
    /// # Errors
    ///
    /// Errors with [`TurnServiceErr::TurnAuthRepoErr`] if authentication in
    /// Redis fails.
    pub fn new(cf: &conf::Turn) -> Result<Self, TurnServiceErr> {
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
            tokio::spawn(async move {
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
                            "Failed to remove IceUser(name: {}) \
                             from Coturn: {}",
                            user,
                            e,
                        );
                    }
                }
            })
        };

        Ok(Service {
            turn_db,
            coturn_cli,
            db_pass: cf.db.redis.pass.to_string(),
            turn_address: cf.addr(),
            turn_username: cf.user.to_string(),
            turn_password: cf.pass.to_string(),
            drop_tx: MpscOneshotSender::from(tx),
            users_cleanup_task,
        })
    }

    /// Returns [`IceUser`] with static credentials.
    fn static_user(&self) -> CoturnIceUser {
        CoturnIceUser::new_static(
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
        let ice_user = CoturnIceUser::new_non_static(
            self.turn_address.clone(),
            &room_id,
            peer_id,
            IcePassword::generate(),
            self.drop_tx.clone(),
        );

        match self.turn_db.insert(&ice_user).await {
            Ok(_) => Ok(ice_user.into()),
            Err(err) => match policy {
                UnreachablePolicy::ReturnErr => Err(err.into()),
                UnreachablePolicy::ReturnStatic => Ok(self.static_user().into()),
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
