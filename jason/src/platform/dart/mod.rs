pub mod constraints;
pub mod error;
pub mod executor;
pub mod ice_candidate;
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

use crate::utils::dart::dart_future::DartFuture;

pub use self::executor::spawn;

type DelayedFutureFunction = extern "C" fn(i32) -> Dart_Handle;
static mut DELAYED_FUTURE_FUNCTION: Option<DelayedFutureFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_delayed_future_function(
    f: DelayedFutureFunction,
) {
    DELAYED_FUTURE_FUNCTION = Some(f);
}

pub async fn delay_for(delay: Duration) {
    let delay = delay.as_millis() as i32;
    let dart_fut = unsafe { DELAYED_FUTURE_FUNCTION.unwrap()(delay) };
    let _ = DartFuture::new(dart_fut).await;
}
