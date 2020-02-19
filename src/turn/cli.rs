use std::fmt;

use deadpool::managed::PoolConfig;
use derive_more::{Display, From};
use failure::Fail;
use medea_coturn_telnet_client::{
    pool::{Manager as PoolManager, Pool, PoolError},
    CoturnTelnetError,
};

use crate::media::IceUser;

#[derive(Display, Debug, Fail, From)]
pub enum CoturnCliError {
    #[display(fmt = "Couldn't get connection from pool: {}", _0)]
    PoolError(PoolError),

    #[display(fmt = "Coturn telnet connection returned error: {}", _0)]
    CliError(CoturnTelnetError),
}

/// Abstraction over remote [Coturn] server telnet interface.
///
/// This struct can be cloned and transferred across thread boundaries.
///
/// [Coturn]: https://github.com/coturn/coturn
#[derive(Clone)]
pub struct CoturnTelnetClient(Pool);

impl CoturnTelnetClient {
    /// Creates new [`CoturnTelnetClient`].
    pub fn new(
        addr: (String, u16),
        pass: String,
        pool_config: PoolConfig,
    ) -> Self {
        Self(Pool::from_config(
            PoolManager::new(addr.0, addr.1, pass),
            pool_config,
        ))
    }

    /// Forcefully closes provided [`IceUser`]s sessions on Coturn server.
    pub async fn delete_sessions(
        &self,
        users: &[&IceUser],
    ) -> Result<(), CoturnCliError> {
        let mut conn = self.0.get().await?;
        for user in users {
            let sessions =
                conn.print_sessions(user.user().clone().into()).await?;
            conn.delete_sessions(sessions).await?;
        }

        Ok(())
    }
}

impl fmt::Debug for CoturnTelnetClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoturnTelnetClient")
            .field("pool", &self.0.status())
            .finish()
    }
}
