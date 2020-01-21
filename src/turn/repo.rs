//! Abstraction over remote Redis database used to store Turn server
//! credentials.
use std::{fmt::Debug, sync::Arc, time::Duration};

use crypto::{digest::Digest, md5::Md5};
use deadpool::managed::{PoolConfig, Timeouts};
use deadpool_redis::{cmd, Pool, PoolError};
use derive_more::Display;
use failure::Fail;
use futures::future::Future;
use redis::{IntoConnectionInfo, RedisError};

use crate::{log::prelude::*, media::IceUser};

#[derive(Debug, Display, Fail)]
pub enum TurnDatabaseErr {
    #[display(fmt = "Could not get connection from pool because: {}", _0)]
    PoolError(PoolError),
    #[display(fmt = "Redis returned error: {}", _0)]
    RedisError(RedisError),
}

impl From<RedisError> for TurnDatabaseErr {
    fn from(err: RedisError) -> Self {
        Self::RedisError(err)
    }
}
impl From<PoolError> for TurnDatabaseErr {
    fn from(err: PoolError) -> Self {
        Self::PoolError(err)
    }
}

// Abstraction over remote Redis database used to store Turn server
// credentials.
pub struct TurnDatabase {
    pool: Arc<Pool>,
}

impl TurnDatabase {
    /// Creates new [`TurnDatabase`].
    pub fn new<S: IntoConnectionInfo + Clone>(
        conn_timeout: Duration,
        conn_info: S,
    ) -> Result<Self, TurnDatabaseErr> {
        let manager = deadpool_redis::Manager::new(conn_info)?;
        let config = PoolConfig {
            max_size: 16,
            timeouts: Timeouts {
                wait: None,
                create: Some(conn_timeout),
                recycle: None,
            },
        };
        let pool = Pool::from_config(manager, config);

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Inserts provided [`IceUser`] into remote Redis database.
    pub fn insert(
        &mut self,
        user: &IceUser,
    ) -> impl Future<Output = Result<(), TurnDatabaseErr>> {
        debug!("Store ICE user: {:?}", user);

        let key = format!("turn/realm/medea/user/{}/key", user.user());
        let value = format!("{}:medea:{}", user.user(), user.pass());

        let pool = Arc::clone(&self.pool);
        async move {
            let mut connection = pool.get().await?;

            let mut hasher = Md5::new();
            hasher.input_str(&value);
            let result = hasher.result_str();

            Ok(cmd("SET")
                .arg(key)
                .arg(result)
                .query_async(&mut connection)
                .await?)
        }
    }

    /// Deletes batch of provided [`IceUser`]s.
    pub fn remove(
        &mut self,
        users: &[IceUser],
    ) -> impl Future<Output = Result<(), TurnDatabaseErr>> {
        debug!("Remove ICE users: {:?}", users);

        let mut delete_keys = Vec::with_capacity(users.len());
        for user in users {
            delete_keys
                .push(format!("turn/realm/medea/user/{}/key", user.user()));
        }
        let pool = Arc::clone(&self.pool);
        async move {
            let mut connection = pool.get().await?;

            Ok(cmd("DEL")
                .arg(delete_keys)
                .query_async(&mut connection)
                .await?)
        }
    }
}

impl Debug for TurnDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "TurnDatabase: {:?}", self.pool.status())
    }
}
