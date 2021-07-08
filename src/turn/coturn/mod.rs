//! Service responsible for managing [Coturn] [TURN] server.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [TURN]: https://webrtcglossary.com/turn

mod allocation_event;
mod cli;
mod coturn_metrics;
mod ice_user;
mod repo;

use std::slice;

use async_trait::async_trait;
use derive_more::Display;
use futures::{channel::mpsc, StreamExt as _};
use medea_client_api_proto::{PeerId, RoomId};
use redis::ConnectionInfo;
use tokio::task::JoinHandle;

use crate::{conf, log::prelude as log, utils::MpscOneshotSender};

use super::{IceUser, TurnAuthService, TurnServiceErr, UnreachablePolicy};

use self::{
    cli::CoturnTelnetClient,
    ice_user::{IcePassword, IceUsername},
    repo::TurnDatabase,
};

pub use self::{
    cli::CoturnCliError, ice_user::CoturnIceUser, repo::TurnDatabaseErr,
};

/// Username of the [Coturn] user.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Clone, Debug, Display, Eq, Hash, PartialEq)]
#[display(fmt = "{}_{}", room_id, peer_id)]
pub struct CoturnUsername {
    /// [`RoomId`] of the `Room` this user is created for.
    pub room_id: RoomId,

    /// [`PeerId`] of the `Peer` this user is created for.
    pub peer_id: PeerId,
}

/// [`TurnAuthService`] implementation backed by [Coturn] [STUN]/[TURN] server.
///
/// [Coturn]: https://github.com/coturn/coturn
/// [STUN]: https://webrtcglossary.com/stun
/// [TURN]: https://webrtcglossary.com/turn
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
    /// Creates a new [`Service`] instance.
    ///
    /// # Errors
    ///
    /// Errors with [`TurnServiceErr::TurnAuthRepoErr`] if authentication in
    /// [Redis] database fails.
    ///
    /// [Redis]: https://redis.io
    pub fn new(cf: &conf::ice::Coturn) -> Result<Self, TurnServiceErr> {
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

    /// Returns an [`IceUser`] with static credentials.
    #[inline]
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
    /// Generates an [`IceUser`] with saved TURN address, provided [`MemberId`]
    /// and random password. Inserts created [`IceUser`] into [`TurnDatabase`].
    ///
    /// [`MemberId`]: medea_client_api_proto::MemberId
    async fn create(
        &self,
        room_id: RoomId,
        peer_id: PeerId,
        policy: UnreachablePolicy,
    ) -> Result<Vec<IceUser>, TurnServiceErr> {
        let ice_user = CoturnIceUser::new_non_static(
            self.turn_address.clone(),
            &room_id,
            peer_id,
            IcePassword::generate(),
            self.drop_tx.clone(),
        );

        match self.turn_db.insert(&ice_user).await {
            Ok(_) => Ok(vec![ice_user.into()]),
            Err(err) => match policy {
                UnreachablePolicy::Error => Err(err.into()),
                UnreachablePolicy::Static => {
                    Ok(vec![self.static_user().into()])
                }
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
