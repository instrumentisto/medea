//! Client library for Medea media server.
//!
//! [Medea]: https://github.com/instrumentisto/medea

// TODO: Remove `clippy::must_use_candidate` once the issue below is resolved:
//       https://github.com/rust-lang/rust-clippy/issues/4779
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]
#![cfg_attr(not(feature = "mockable"), warn(missing_docs))]
#![cfg_attr(feature = "mockable", allow(missing_docs))]

#[macro_use]
pub mod utils;

pub mod api;
pub mod media;
pub mod peer;
pub mod rpc;

// When the `console_error_panic_hook` feature is enabled, we can call the
// `set_panic_hook` function at least once during initialization, and then
// we will get better error messages if our code ever panics.
//
// For more details see:
// https://github.com/rustwasm/console_error_panic_hook#readme
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

#[doc(inline)]
pub use self::{
    api::{ConnectionHandle, Jason, RoomHandle},
    media::{
        AudioTrackConstraints, DeviceVideoTrackConstraints,
        DisplayVideoTrackConstraints, FacingMode, MediaTracksSettings,
    },
};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
