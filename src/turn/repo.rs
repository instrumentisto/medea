//! Abstraction over remote Redis database used to store Turn server
//! credentials.

use std::{fmt, time::Duration};

use crypto::{digest::Digest, md5::Md5};
use deadpool::managed::{PoolConfig, Timeouts};
use deadpool_redis::{cmd, Pool, PoolError};
use derive_more::{Display, From};
use failure::Fail;
use redis::{IntoConnectionInfo, RedisError};

use crate::{log::prelude::*, media::IceUser};

#[derive(Debug, Display, Fail, From)]
pub enum TurnDatabaseErr {
    #[display(fmt = "Couldn't get connection from pool: {}", _0)]
    PoolError(PoolError),

    #[display(fmt = "Redis returned error: {}", _0)]
    RedisError(RedisError),
}

/// Abstraction over remote Redis database used to store Turn server
/// credentials.
///
/// This struct can be cloned and transferred across thread boundaries.
#[derive(Clone)]
pub struct TurnDatabase {
    pool: Pool,
}

impl TurnDatabase {
    /// Creates new [`TurnDatabase`].
    pub fn new<S: IntoConnectionInfo + Clone>(
        conn_timeout: Duration,
        conn_info: S,
    ) -> Result<Self, TurnDatabaseErr> {
        let manager = deadpool_redis::Manager::new(conn_info)?;
        let config = PoolConfig {
            max_size: 16, // TODO: configure via conf
            timeouts: Timeouts {
                wait: None,
                create: Some(conn_timeout),
                recycle: None,
            },
        };
        Ok(Self {
            pool: Pool::from_config(manager, config),
        })
    }

    /// Inserts provided [`IceUser`] into remote Redis database.
    pub async fn insert(&self, user: &IceUser) -> Result<(), TurnDatabaseErr> {
        debug!("Store ICE user: {:?}", user);

        let key = format!("turn/realm/medea/user/{}/key", user.user());
        let value = format!("{}:medea:{}", user.user(), user.pass());

        let mut hasher = Md5::new();
        hasher.input_str(&value);
        let result = hasher.result_str();

        let mut conn = self.pool.get().await?;
        Ok(cmd("SET")
            .arg(key)
            .arg(result)
            .query_async(&mut conn)
            .await?)
    }

    /// Deletes batch of provided [`IceUser`]s.
    ///
    /// No-op if empty batch is provided.
    pub async fn remove(
        &self,
        users: &[&IceUser],
    ) -> Result<(), TurnDatabaseErr> {
        debug!("Remove ICE users: {:?}", users);

        if users.is_empty() {
            return Ok(());
        }

        let keys: Vec<_> = users
            .iter()
            .map(|u| format!("turn/realm/medea/user/{}/key", u.user()))
            .collect();

        let mut conn = self.pool.get().await?;
        Ok(cmd("DEL").arg(keys).query_async(&mut conn).await?)
    }
}

impl fmt::Debug for TurnDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnDatabase")
            .field("pool", &self.pool.status())
            .finish()
    }
}
