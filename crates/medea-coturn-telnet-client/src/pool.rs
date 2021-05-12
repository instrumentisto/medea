//! [deadpool]-based simple async pool for [`CoturnTelnetConnection`]s.
//!
//! You shouldn't use [deadpool] directly, just use the [`Pool`] type provided
//! by this crate instead.
//!
//! # Example
//!
//! ```rust,should_panic
//! use std::ops::DerefMut as _;
//! use medea_coturn_telnet_client::pool::{Manager, Pool};
//!
//! #[tokio::main]
//! async fn main() {
//!     let pool = Pool::new(Manager::new("localhost", 1234, "turn"), 16);
//!     pool.get()
//!         .await
//!         .expect("Failed connect to TURN server")
//!         .print_sessions(String::from("username"))
//!         .await
//!         .expect("Failed to print sessions");
//! }
//! ```
//!
//! [deadpool]: https://crates.io/crates/deadpool

use async_trait::async_trait;
use bytes::Bytes;
use deadpool::managed;

use crate::client::{CoturnTelnetConnection, CoturnTelnetError};

/// Type alias for using a [`deadpool::managed::Pool`] with a
/// [`CoturnTelnetConnection`].
pub type Pool = managed::Pool<Manager>;

/// Type alias for using a [`deadpool::managed::PoolError`] with a
/// [`CoturnTelnetConnection`].
pub type Error = managed::PoolError<CoturnTelnetError>;

/// Type alias for using a [`deadpool::managed::Object`] with a
/// [`CoturnTelnetConnection`].
pub type Connection = managed::Object<Manager>;

/// Type alias for using [`deadpool::managed::RecycleResult`] with a
/// [`CoturnTelnetError`].
type RecycleResult = managed::RecycleResult<CoturnTelnetError>;

/// Manager for creating and recycling [`CoturnTelnetConnection`]s.
#[derive(Debug)]
pub struct Manager {
    /// Host and port of the server to establish connections onto.
    addr: (String, u16),

    /// Password to authenticate connections with.
    pass: Bytes,
}

impl Manager {
    /// Creates a new [`Manager`] with the given credentials.
    #[inline]
    #[must_use]
    pub fn new<S, P>(host: S, port: u16, pass: P) -> Self
    where
        S: Into<String>,
        P: Into<Bytes>,
    {
        Self {
            addr: (host.into(), port),
            pass: pass.into(),
        }
    }
}

#[async_trait]
impl managed::Manager for Manager {
    type Error = CoturnTelnetError;
    type Type = CoturnTelnetConnection;

    #[inline]
    async fn create(&self) -> Result<Self::Type, Self::Error> {
        Self::Type::connect(
            (self.addr.0.as_str(), self.addr.1),
            self.pass.clone(),
        )
        .await
    }

    #[inline]
    async fn recycle(&self, conn: &mut Self::Type) -> RecycleResult {
        conn.ping().await.map_err(Into::into)
    }
}
