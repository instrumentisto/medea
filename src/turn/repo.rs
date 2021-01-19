//! Abstraction over remote Redis database used to store Turn server
//! credentials.

use std::{fmt, time::Duration};

use crypto::{digest::Digest, md5::Md5};
use deadpool::managed::{PoolConfig, Timeouts};
use deadpool_redis::{cmd, Pool, PoolError};
use derive_more::{Display, From};
use failure::Fail;
use futures::future::BoxFuture;
use redis::{IntoConnectionInfo, RedisError};

use crate::{
    log::prelude::*,
    turn::{ice_user::IcePassword, IceUsername, TurnDatabase, COTURN_REALM},
};

#[derive(Debug, Display, Fail, From)]
pub enum TurnDatabaseErr {
    #[display(fmt = "Couldn't get connection from pool: {}", _0)]
    PoolError(PoolError),

    #[display(fmt = "Redis returned error: {}", _0)]
    RedisError(RedisError),
}

/// Returns [Coturn]'s [HMAC key] for the provided [`IceUsername`] and
/// [`IcePassword`].
///
/// [HMAC key]: https://tinyurl.com/y33qa86c
fn hmackey(username: &IceUsername, pass: &IcePassword) -> String {
    let mut hasher = Md5::new();
    hasher.input_str(&format!("{}:{}:{}", username, COTURN_REALM, pass));
    hasher.result_str()
}

/// Abstraction over remote Redis database used to store Turn server
/// credentials.
///
/// This struct can be cloned and transferred across thread boundaries.
#[derive(Clone)]
pub struct RedisTurnDatabase {
    pool: Pool,
}

impl RedisTurnDatabase {
    /// Creates new [`TurnDatabase`].
    ///
    /// # Errors
    ///
    /// Errors if authentication in Redis fails.
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
}

impl fmt::Debug for RedisTurnDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnDatabase")
            .field("pool", &self.pool.status())
            .finish()
    }
}

impl TurnDatabase for RedisTurnDatabase {
    fn insert(
        &self,
        username: &IceUsername,
        pass: &IcePassword,
    ) -> BoxFuture<'static, Result<(), TurnDatabaseErr>> {
        let pool = self.pool.clone();
        let key = username.as_redis_key();
        let value = hmackey(&username, &pass);
        Box::pin(async move {
            debug!("Store ICE user with a key: {}", key);

            let mut conn = pool.get().await?;
            Ok(cmd("SET")
                .arg(key)
                .arg(value)
                .query_async(&mut conn)
                .await?)
        })
    }

    fn remove(
        &self,
        username: &IceUsername,
    ) -> BoxFuture<'static, Result<(), TurnDatabaseErr>> {
        let pool = self.pool.clone();
        let key = username.as_redis_key();
        Box::pin(async move {
            debug!("Remove ICE user with a key: {}", key);

            let mut conn = pool.get().await?;
            Ok(cmd("DEL").arg(key).query_async(&mut conn).await?)
        })
    }
}
