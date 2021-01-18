//! Abstraction over remote Redis database used to store Turn server
//! credentials.

use std::{fmt, time::Duration};

use crypto::{digest::Digest, md5::Md5};
use deadpool::managed::{PoolConfig, Timeouts};
use deadpool_redis::{cmd, Pool, PoolError};
use derive_more::{Display, From};
use failure::Fail;
use redis::{IntoConnectionInfo, RedisError};

use crate::{
    log::prelude::*,
    turn::{ice_user::IcePassword, IceUsername, COTURN_REALM},
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
pub struct TurnDatabase {
    pool: Pool,
}

impl TurnDatabase {
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

    /// Inserts provided [`IceUsername`] and [`IcePassword`] into remote Redis
    /// database.
    ///
    /// # Errors
    ///
    /// Errors if unable to establish connection with database, or database
    /// request fails.
    pub async fn insert(
        &self,
        username: &IceUsername,
        pass: &IcePassword,
    ) -> Result<(), TurnDatabaseErr> {
        debug!("Store ICE user: {}", username);

        let mut conn = self.pool.get().await?;
        Ok(cmd("SET")
            .arg(username.as_redis_key())
            .arg(hmackey(&username, &pass))
            .query_async(&mut conn)
            .await?)
    }

    /// Deletes provided [`IceUsername`].
    ///
    /// # Errors
    ///
    /// Errors if unable to establish connection with database, or database
    /// request fails.
    pub async fn remove(
        &self,
        username: &IceUsername,
    ) -> Result<(), TurnDatabaseErr> {
        debug!("Remove ICE user: {}", username);

        let mut conn = self.pool.get().await?;
        Ok(cmd("DEL")
            .arg(username.as_redis_key())
            .query_async(&mut conn)
            .await?)
    }
}

impl fmt::Debug for TurnDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnDatabase")
            .field("pool", &self.pool.status())
            .finish()
    }
}
