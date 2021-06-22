//! Client library for [Medea] media server.
//!
//! [Medea]: https://github.com/instrumentisto/medea

#![allow(clippy::module_name_repetitions)]
#![deny(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
#![forbid(non_ascii_idents)]
#![cfg_attr(not(feature = "mockable"), warn(missing_docs))]
#![cfg_attr(feature = "mockable", allow(missing_docs))]

#[macro_use]
pub mod utils;
pub mod api;
pub mod connection;
pub mod jason;
pub mod media;
pub mod peer;
pub mod platform;
pub mod room;
pub mod rpc;
