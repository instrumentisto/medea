//! [Coturn] server admin [Telnet] interface client.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [Telnet]: https://en.wikipedia.org/wiki/Telnet

use std::fmt;

use bytes::Bytes;
use deadpool::managed::PoolConfig;
use derive_more::{Display, From};
use failure::Fail;
use medea_coturn_telnet_client::{
    pool::{Error as PoolError, Manager as PoolManager, Pool},
    CoturnTelnetError,
};

use crate::turn::IceUsername;

/// Possible errors returned by [`CoturnTelnetClient`].
#[derive(Display, Debug, Fail, From)]
pub enum CoturnCliError {
    /// Failed to retrieve connection from pool.
    #[display(fmt = "Cannot retrieve connection from pool: {}", _0)]
    PoolError(PoolError),

    /// Operation on retrieved connection failed.
    #[display(fmt = "Connection returned error: {}", _0)]
    CliError(CoturnTelnetError),
}

/// Abstraction over remote [Coturn] server admin [Telnet] interface.
///
/// This struct can be cloned and transferred across thread boundaries.
///
/// [Coturn]: https://github.com/coturn/coturn
/// [Telnet]: https://en.wikipedia.org/wiki/Telnet
#[derive(Clone)]
pub struct CoturnTelnetClient(Pool);

impl CoturnTelnetClient {
    /// Creates new [`CoturnTelnetClient`] with the provided configuration.
    #[inline]
    #[must_use]
    pub fn new<H: Into<String>, P: Into<Bytes>>(
        addr: (H, u16),
        pass: P,
        pool_config: PoolConfig,
    ) -> Self {
        Self(Pool::from_config(
            PoolManager::new(addr.0, addr.1, pass),
            pool_config,
        ))
    }

    /// Forcibly closes sessions on [Coturn] server by the provided
    /// [`IceUsername`]s.
    ///
    /// # Errors
    ///
    /// When:
    /// - establishing connection with [Coturn] fails;
    /// - retrieving all `users`' sessions from [Coturn] fails;
    /// - deleting all retrieved `users`' sessions fails.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    ///
    /// # Errors
    ///
    /// With [`CoturnCliError::PoolError`] if could not get or establish new
    /// connection in pool.
    ///
    /// With [`CoturnCliError::CliError`] in case of unexpected protocol error.
    pub async fn delete_sessions(
        &self,
        users: &[IceUsername],
    ) -> Result<(), CoturnCliError> {
        let mut conn = self.0.get().await?;
        for u in users {
            let sessions =
                conn.print_sessions(u.to_string()).await?.into_iter();
            conn.delete_sessions(sessions).await?;
        }
        Ok(())
    }
}

impl fmt::Debug for CoturnTelnetClient {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoturnTelnetClient")
            .field("pool", &self.0.status())
            .finish()
    }
}
