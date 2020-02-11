//! Implements client to access [Coturn] telnet cli. You can use
//! [`CoturnTelnetConnection`] directly, but it is recommended to use connection
//! pool based on [deadpool] that will take care of connection lifecycle.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [deadpool]: https://crates.io/crates/deadpool
#![allow(clippy::module_name_repetitions)]

pub mod codec;
pub mod connection;
pub mod pool;

pub use connection::{CoturnTelnetConnection, CoturnTelnetError};
pub use pool::{Connection, Manager, Pool, PoolError};
