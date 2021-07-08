//! Abstraction over remote Redis database used to store Turn server
//! credentials.

use std::{fmt, time::Duration};

use crypto::{digest::Digest, md5::Md5};
use deadpool::{managed::Timeouts, Runtime};
use deadpool_redis::{redis::cmd, Pool, PoolConfig, PoolError};
use derive_more::{Display, From};
use failure::Fail;
use redis::{IntoConnectionInfo, RedisError};

use crate::log::prelude as log;

use super::ice_user::{CoturnIceUser, IceUsername};

/// Medea's [Coturn] realm name.
const COTURN_REALM: &str = "medea";

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
pub struct TurnDatabase(Pool);

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
            runtime: Runtime::Tokio1,
        };
        Ok(Self(Pool::from_config(manager, config)))
    }

    /// Inserts provided [`IceUser`] into remote Redis database.
    ///
    /// # Errors
    ///
    /// Errors if unable to establish connection with database, or database
    /// request fails.
    pub async fn insert(
        &self,
        user: &CoturnIceUser,
    ) -> Result<(), TurnDatabaseErr> {
        log::debug!("Store ICE user: {:?}", user);

        let key = user.user().redis_key();
        let value = user.redis_hmac_key();

        let mut conn = self.0.get().await?;
        Ok(cmd("SET")
            .arg(key)
            .arg(value)
            .query_async(&mut conn)
            .await?)
    }

    /// Deletes batch of provided [`IceUser`]s.
    ///
    /// No-op if empty batch is provided.
    ///
    /// # Errors
    ///
    /// Errors if unable to establish connection with database, or database
    /// request fails.
    pub async fn remove(
        &self,
        users: &[IceUsername],
    ) -> Result<(), TurnDatabaseErr> {
        log::debug!("Remove ICE users: {:?}", users);

        if users.is_empty() {
            return Ok(());
        }

        let keys: Vec<_> = users.iter().map(IceUsername::redis_key).collect();

        let mut conn = self.0.get().await?;
        Ok(cmd("DEL").arg(keys).query_async(&mut conn).await?)
    }
}

impl IceUsername {
    /// Forms a Redis key of this [`IceUsername`].
    #[must_use]
    fn redis_key(&self) -> String {
        format!("turn/realm/{}/user/{}/key", COTURN_REALM, self)
    }
}

impl CoturnIceUser {
    /// Forms a [Coturn]'s [HMAC key] of this [`IceUser`].
    ///
    /// [HMAC key]: https://tinyurl.com/y33qa86c
    #[must_use]
    fn redis_hmac_key(&self) -> String {
        let mut hasher = Md5::new();
        hasher.input_str(&format!(
            "{}:{}:{}",
            self.user(),
            COTURN_REALM,
            self.pass()
        ));
        hasher.result_str()
    }
}

impl fmt::Debug for TurnDatabase {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnDatabase")
            .field("pool", &self.0.status())
            .finish()
    }
}
