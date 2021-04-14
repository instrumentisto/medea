//! Client library for [Medea] media server.
//!
//! [Medea]: https://github.com/instrumentisto/medea

#![allow(clippy::module_name_repetitions)]
#![deny(broken_intra_doc_links)]
// #![cfg_attr(not(feature = "mockable"), warn(missing_docs))]
// #![cfg_attr(feature = "mockable", allow(missing_docs))]

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
