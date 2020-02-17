//! Implements client to access [Coturn] telnet cli. You can use
//! [`CoturnTelnetConnection`] directly, but it is recommended to use connection
//! pool based on [deadpool] that will take care of connection lifecycle.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [deadpool]: https://crates.io/crates/deadpool

#![deny(
    intra_doc_link_resolution_failure,
    missing_debug_implementations,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts
)]
#![forbid(unsafe_code)]
#![warn(
    deprecated_in_future,
    missing_copy_implementations,
    missing_docs,
    unreachable_pub,
    unused_import_braces,
    unused_labels,
    unused_lifetimes,
    unused_qualifications,
    unused_results
)]

pub mod client;
pub mod con_pool;
pub mod framed;

#[doc(inline)]
pub use client::{CoturnTelnetConnection, CoturnTelnetError};
#[doc(inline)]
pub use con_pool::{Connection, Manager, Pool, PoolError};
