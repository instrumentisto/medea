//! Miscellaneous utility structs and functions.

mod callback;
#[macro_use]
mod errors;
mod event_listener;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use web_sys::Window;

#[doc(inline)]
pub use self::{
    callback::{Callback, Callback2},
    errors::{JasonError, JsCaused, WasmErr},
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

/// Upgrades newtyped [`Weak`] reference, returning [`WasmErr`] if failed,
/// or mapping [`Rc`]-referenced value with provided `$closure` otherwise.
///
/// [`Rc`]: std::rc::Rc
/// [`Weak`]: std::rc::Weak
macro_rules! map_weak {
    ($v:expr, $closure:expr) => {{
        $v.0.upgrade()
            .ok_or_else(|| js_sys::Error::new("Detached state").into())
            .map($closure)
    }};
}

macro_rules! error {
    ($e:expr) => {
        web_sys::console::error_1(&$e.into())
    };
}

/// Returns property of JS object by name if its defined.
/// Converts the value with a given predicate.
pub fn get_property_by_name<T, F, U>(
    value: &T,
    name: &str,
    into: F,
) -> Option<U>
where
    T: AsRef<wasm_bindgen::JsValue>,
    F: Fn(wasm_bindgen::JsValue) -> Option<U>,
{
    Reflect::get(value.as_ref(), &JsValue::from_str(name))
        .ok()
        .map_or_else(|| None, into)
}
