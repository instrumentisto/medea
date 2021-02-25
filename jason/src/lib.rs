//! Client library for Medea media server.
//!
//! [Medea]: https://github.com/instrumentisto/medea

// TODO: Remove `clippy::must_use_candidate` once the issue below is resolved:
//       https://github.com/rust-lang/rust-clippy/issues/4779
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]
#![deny(broken_intra_doc_links)]
#![cfg_attr(not(feature = "mockable"), warn(missing_docs))]
#![cfg_attr(feature = "mockable", allow(missing_docs))]

pub use self::core::utils;

pub mod api;
pub mod core;
pub mod platform;
