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

use std::{panic, time::Duration};

use dart_sys::Dart_Handle;

use crate::{
    platform::dart::utils::dart_api::Dart_PropagateError_DL_Trampolined,
    utils::dart::{
        dart_future::{DartFuture, VoidDartFuture},
        into_dart_string,
    },
};

pub use self::executor::spawn;

type NewExceptionFunction = extern "C" fn(*const libc::c_char) -> Dart_Handle;
static mut NEW_EXCEPTION_FUNCTION: Option<NewExceptionFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_new_exception_function(
    f: NewExceptionFunction,
) {
    NEW_EXCEPTION_FUNCTION = Some(f);
}

pub fn set_panic_hook() {
    panic::set_hook(Box::new(|s| {
        let exception = unsafe {
            NEW_EXCEPTION_FUNCTION.unwrap()(into_dart_string(s.to_string()))
        };
        unsafe { Dart_PropagateError_DL_Trampolined(exception) };
    }));
}

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
    let _ = VoidDartFuture::new(dart_fut).await;
}
