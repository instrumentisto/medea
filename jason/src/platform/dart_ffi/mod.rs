//! Multiplatform Dart runtime specific functionality.

#![allow(
    unused_variables,
    clippy::missing_panics_doc,
    clippy::unused_self,
    clippy::needless_pass_by_value
)]

use std::time::Duration;

pub mod constraints;
pub mod error;
pub mod ice_server;
pub mod input_device_info;
pub mod media_devices;
pub mod media_track;
pub mod peer_connection;
pub mod rtc_stats;
pub mod transceiver;
pub mod transport;
pub mod utils;

use futures::Future;

/// TODO: Implement panic hook.
pub fn set_panic_hook() {}

/// Initialize [`android_logger`] as default application logger with min log
/// level set to [`log::Level::Debug`].
///
/// [`android_logger`]: https://docs.rs/android_logger
pub fn init_logger() {
    android_logger::init_once(
        android_logger::Config::default().with_min_level(log::Level::Debug),
    );
}

/// Runs a Rust [`Future`] on the current thread.
#[inline]
pub fn spawn<F>(task: F)
where
    F: Future<Output = ()> + 'static,
{
    unimplemented!()
}

/// [`Future`] which resolves after the provided [`Duration`].
///
/// [`Future`]: std::future::Future
pub async fn delay_for(delay: Duration) {
    unimplemented!()
}
