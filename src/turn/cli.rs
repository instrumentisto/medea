use std::{fmt, ops::DerefMut};

use deadpool::managed::PoolConfig;
use derive_more::{Display, From};
use failure::Fail;
use medea_coturn_telnet::{CoturnTelnetError, Manager, Pool, PoolError};

use crate::media::IceUser;
use medea_coturn_telnet::sessions_parser::Session;

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
        Self(Pool::from_config(Manager::new(addr, pass), pool_config))
    }

    /// Forcefully closes provided [`IceUser`]s sessions on Coturn server.
    pub async fn delete_sessions(
        &self,
        users: &[&IceUser],
    ) -> Result<(), CoturnCliError> {
        let mut connection = self.0.get().await?;
        for user in users {
            let sessions = connection
                .deref_mut()
                .print_sessions(user.user().clone().into())
                .await?;
            let sessions_ids = sessions.into_iter().map(|session| session.id);
            connection.deref_mut().delete_sessions(sessions_ids).await?;
        }

        Ok(())
    }

    pub async fn get_sessions(
        &self,
        username: String,
    ) -> Result<Vec<Session>, CoturnCliError> {
        let mut connection = self.0.get().await?;

        Ok(connection.print_sessions(username).await?)
    }
}

impl fmt::Debug for CoturnTelnetClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoturnTelnetClient")
            .field("pool", &self.0.status())
            .finish()
    }
}
