//! [Telnet] client implementation to access [Coturn] admin interface (cli).
//!
//! You may use [`CoturnTelnetConnection`] directly, but it is recommended
//! to use connections pool (based on [deadpool]) that will take care of
//! connections lifecycle. Enable `pool` feature for that.
//!
//! [Coturn]: https://github.com/coturn/coturn
//! [deadpool]: https://crates.io/crates/deadpool
//! [Telnet]: https://en.wikipedia.org/wiki/Telnet

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(
    missing_debug_implementations,
    nonstandard_style,
    rust_2018_idioms,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
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
#[cfg(feature = "pool")]
#[cfg_attr(docsrs, doc(cfg(feature = "pool")))]
pub mod pool;
pub mod proto;

#[doc(inline)]
pub use self::client::{CoturnTelnetConnection, CoturnTelnetError};
