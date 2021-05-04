//! Multiplatform Dart runtime specific functionality.

// TODO: Remove allows when implementing platform code.
#![allow(
    unused_variables,
    clippy::missing_panics_doc,
    clippy::unused_self,
    clippy::needless_pass_by_value
)]

pub mod constraints;
pub mod error;
pub mod executor;
pub mod ice_server;
pub mod input_device_info;
pub mod media_devices;
pub mod media_track;
pub mod peer_connection;
pub mod rtc_stats;
pub mod transceiver;
pub mod transport;
pub mod utils;

use std::time::Duration;

use dart_sys::Dart_Handle;

pub use self::{
    constraints::{DisplayMediaStreamConstraints, MediaStreamConstraints},
    error::Error,
    executor::spawn,
    input_device_info::InputDeviceInfo,
    media_devices::{enumerate_devices, get_display_media, get_user_media},
    media_track::MediaStreamTrack,
    peer_connection::RtcPeerConnection,
    rtc_stats::RtcStats,
    transceiver::{Transceiver, TransceiverDirection},
    transport::WebSocketRpcTransport,
    utils::{dart_future_to_rust, Callback, Function},
};

/// TODO: Implement panic hook.
pub fn set_panic_hook() {}

/// Initialize [`android_logger`] as default application logger with min log
/// level set to [`log::Level::Debug`].
///
/// [`android_logger`]: https://docs.rs/android_logger
pub fn init_logger() {
    // TODO: android_logger::init_once should be called only once.
    android_logger::init_once(
        android_logger::Config::default().with_min_level(log::Level::Debug),
    );
}

/// Pointer to an extern function that returns Dart `Future` which will be
/// resolved after provided number of milliseconds.
type DelayedFutureCaller = extern "C" fn(i32) -> Dart_Handle;

/// Stores pointer to the [`DelayerFutureCaller`] extern function.
///
/// Should be initialized by Dart during FFI initialization phase.
static mut DELAYED_FUTURE_CALLER: Option<DelayedFutureCaller> = None;

/// Registers the provided [`DelayedFutureCaller`] as
/// [`DELAYER_FUTURE_CALLER`].
///
/// # Safety
///
/// Must ONLY be called by Dart during FFI initialization.
#[no_mangle]
pub unsafe extern "C" fn register_delayed_future_caller(
    f: DelayedFutureCaller,
) {
    DELAYED_FUTURE_CALLER = Some(f);
}

/// [`Future`] which resolves after the provided [`Duration`].
///
/// [`Future`]: std::future::Future
pub async fn delay_for(delay: Duration) {
    let delay = delay.as_millis() as i32;
    let dart_fut = unsafe { DELAYED_FUTURE_CALLER.unwrap()(delay) };
    let _ = dart_future_to_rust(dart_fut).await;
}
