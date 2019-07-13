//! Miscellaneous utility structs and functions.

mod callback;
mod errors;
mod event_listener;

use web_sys::Window;

#[doc(inline)]
pub use self::{
    callback::{Callback, Callback2},
    errors::WasmErr,
    event_listener::EventListener,
};

/// Returns [`Window`] object.
///
/// # Panics
///
/// When global [`Window`] object is inaccessible.
pub fn window() -> Window {
    // Cannot use `lazy_static` since `window` is `!Sync`.
    // Safe to unwrap.
    web_sys::window().unwrap()
}

/// Wrapper around interval timer ID.
pub struct IntervalHandle(pub i32);

impl Drop for IntervalHandle {
    /// Clears interval with provided ID.
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.0);
    }
}
