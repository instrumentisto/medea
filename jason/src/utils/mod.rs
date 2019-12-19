//! Miscellaneous utility structs and functions.

#[macro_use]
mod errors;

mod callback;
mod event_listener;

use std::time::Duration;

use derive_more::{Add, From, Mul, Sub};
use js_sys::{Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Window;

#[doc(inline)]
pub use self::{
    callback::{Callback, Callback2},
    errors::{
        console_error, HandlerDetachedError, JasonError, JsCaused, JsError,
    },
    event_listener::{EventListener, EventListenerBindError},
};
use std::rc::{Rc, Weak};

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

#[derive(Debug, From, Copy, Clone, Add, Mul, Sub)]
pub struct JsDuration(Duration);

impl JsDuration {
    pub fn into_js_duration(self) -> i32 {
        self.0.as_millis() as i32
    }
}

/// Wrapper around interval timer ID.
pub struct IntervalHandle(pub i32);

impl Drop for IntervalHandle {
    /// Clears interval with provided ID.
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.0);
    }
}

pub trait JasonWeakHandler<I> {
    fn upgrade_handler<E>(&self) -> Result<Rc<I>, E>
    where
        E: From<JasonError>;
}

impl<I> JasonWeakHandler<I> for Weak<I> {
    fn upgrade_handler<E>(&self) -> Result<Rc<I>, E>
    where
        E: From<JasonError>,
    {
        self.upgrade()
            .ok_or(JasonError::from(tracerr::new!(HandlerDetachedError)).into())
    }
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

/// Resolves after provided [`JsDuration`].
pub async fn resolve_after(delay_ms: JsDuration) -> Result<(), JsValue> {
    JsFuture::from(Promise::new(&mut |yes, _| {
        window()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &yes,
                delay_ms.into_js_duration(),
            )
            .unwrap();
    }))
    .await?;
    Ok(())
}
