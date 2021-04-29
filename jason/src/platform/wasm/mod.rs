//! `wasm32`-platform-specific functionality.

use std::{convert::TryInto as _, time::Duration};

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
use js_sys::{Promise, Reflect};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::Window;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// When the `console_error_panic_hook` feature is enabled, we can call the
// `set_panic_hook` function at least once during initialization, and then
// we will get better error messages if our code ever panics.
//
// For more details see:
// https://github.com/rustwasm/console_error_panic_hook#readme
// #[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

/// Initialize [`wasm_logger`] as default application logger.
///
/// [`wasm_logger`]: https://docs.rs/wasm-logger
pub fn init_logger() {
    wasm_logger::init(wasm_logger::Config::default());
}

/// Runs a Rust [`Future`] on the current thread.
#[inline]
pub fn spawn<F>(task: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(task);
}

/// [`Future`] which resolves after the provided [`Duration`].
///
/// # Panics
///
/// If fails to interact with JS side.
///
/// [`Future`]: std::future::Future
pub async fn delay_for(delay: Duration) {
    let delay_ms = delay.as_millis().try_into().unwrap_or(i32::max_value());
    JsFuture::from(Promise::new(&mut |yes, _| {
        window()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &yes, delay_ms,
            )
            .unwrap();
    }))
    .await
    .unwrap();
}

/// Returns property of JS object by name if its defined.
/// Converts the value with a given predicate.
fn get_property_by_name<T, F, U>(value: &T, name: &str, into: F) -> Option<U>
where
    T: AsRef<wasm_bindgen::JsValue>,
    F: Fn(wasm_bindgen::JsValue) -> Option<U>,
{
    Reflect::get(value.as_ref(), &JsValue::from_str(name))
        .ok()
        .map_or_else(|| None, into)
}

/// Returns [`Window`] object.
///
/// # Panics
///
/// When global [`Window`] object is inaccessible.
#[must_use]
fn window() -> Window {
    // Cannot use `lazy_static` since `window` is `!Sync`.
    // Safe to unwrap.
    web_sys::window().unwrap()
}

/// Wrapper around interval timer ID.
struct IntervalHandle(pub i32);

impl Drop for IntervalHandle {
    /// Clears interval with provided ID.
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.0);
    }
}
