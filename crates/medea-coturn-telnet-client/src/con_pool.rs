//! Deadpool simple async pool for [`CoturnTelnetConnection`]'s.
//!
//! You should not need to use `deadpool` directly. Use the `Pool` type
//! provided by this crate instead.
//!
//! # Example
//!
//! ```rust,should_panic
//! use std::ops::DerefMut;
//! use medea_coturn_telnet_client::{Manager, Pool};
//!
//! let mut rt = tokio::runtime::Runtime::new().unwrap();
//! rt.block_on(async {
//!     let mgr = Manager::new((String::from("localhost"), 1234), "turn");
//!     let pool = Pool::new(mgr, 16);
//!
//!     let mut conn = pool.get().await.unwrap();
//!
//!     conn.deref_mut()
//!         .print_sessions(String::from("username"))
//!         .await
//!         .unwrap();
//! });
//! ```

use async_trait::async_trait;
use bytes::Bytes;
use deadpool::managed;

use crate::client::{CoturnTelnetConnection, CoturnTelnetError};

/// A type alias for using `deadpool::managed::Pool` with
/// [`CoturnTelnetConnection`].
pub type Pool = managed::Pool<CoturnTelnetConnection, CoturnTelnetError>;

/// A type alias for using `deadpool::managed::PoolError` with
/// [`CoturnTelnetConnection`].
pub type PoolError = managed::PoolError<CoturnTelnetError>;

/// A type alias for using `deadpool::managed::Object` with
/// [`CoturnTelnetConnection`].
pub type Connection =
    managed::Object<CoturnTelnetConnection, CoturnTelnetError>;

type RecycleResult = managed::RecycleResult<CoturnTelnetError>;

/// The manager for creating and recycling Coturn telnet connections.
#[derive(Debug)]
pub struct Manager {
    addr: (String, u16),
    pass: Bytes,
}

impl Manager {
    /// Creates [`Manager`].
    pub fn new<P: Into<Bytes>>(addr: (String, u16), pass: P) -> Self {
        Self {
            addr,
            pass: pass.into(),
        }
    }
}

#[async_trait]
impl managed::Manager<CoturnTelnetConnection, CoturnTelnetError> for Manager {
    async fn create(
        &self,
    ) -> Result<CoturnTelnetConnection, CoturnTelnetError> {
        let connection = CoturnTelnetConnection::connect(
            (self.addr.0.as_str(), self.addr.1),
            self.pass.clone(),
        )
        .await?;
        Ok(connection)
    }

    async fn recycle(
        &self,
        connection: &mut CoturnTelnetConnection,
    ) -> RecycleResult {
        connection.ping().await.map_err(From::from)
    }
}
