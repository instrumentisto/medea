use std::{fmt, ops::DerefMut, time::Duration};

use coturn_telnet::{CoturnTelnetError, Manager, Pool, PoolError};
use deadpool::managed::{PoolConfig, Timeouts};
use derive_more::{Display, From};
use failure::Fail;

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
pub struct CoturnTelnetClient {
    pool: Pool,
}

impl CoturnTelnetClient {
    /// Creates new [`CoturnTelnetClient`].
    pub fn new(addr: (String, u16), pass: String) -> Self {
        let manager = Manager::new(addr, pass);
        // TODO: to conf
        let config = PoolConfig {
            max_size: 16,
            timeouts: Timeouts {
                wait: Some(Duration::from_secs(5)),
                create: Some(Duration::from_secs(5)),
                recycle: Some(Duration::from_secs(5)),
            },
        };
        Self {
            pool: Pool::from_config(manager, config),
        }
    }

    /// Forcefully closes provided [`IceUser`]s sessions on Coturn server.
    pub async fn delete_sessions(
        &self,
        users: &[&IceUser],
    ) -> Result<(), CoturnCliError> {
        let mut connection = self.pool.get().await?;
        for user in users {
            let sessions = connection
                .deref_mut()
                .print_sessions(user.user().clone().into())
                .await?;
            connection.deref_mut().delete_sessions(sessions).await?;
        }

        Ok(())
    }
}

impl fmt::Debug for CoturnTelnetClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoturnTelnetClient")
            .field("pool", &self.pool.status())
            .finish()
    }
}
