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

use crate::{media::IceUser, turn::CoturnUsername};

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

    /// Forcibly closes provided [`IceUser`]s sessions on [Coturn] server.
    ///
    /// [Coturn]: https://github.com/coturn/coturn
    pub async fn delete_sessions(
        &self,
        users: &[&IceUser],
    ) -> Result<(), CoturnCliError> {
        let mut conn = self.0.get().await?;
        for u in users {
            let sessions = conn
                .print_sessions(u.user().clone().into())
                .await?
                .into_iter();
            conn.delete_sessions(sessions).await?;
        }
        Ok(())
    }

    /// Lists sessions by [`CoturnUsername`].
    pub async fn get_sessions(
        &self,
        username: CoturnUsername,
    ) -> Result<Vec<String>, CoturnCliError> {
        let mut connection = self.0.get().await?;

        Ok(connection.print_sessions(username.to_string()).await?)
    }
}

impl fmt::Debug for CoturnTelnetClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoturnTelnetClient")
            .field("pool", &self.0.status())
            .finish()
    }
}
