//! Miscellaneous utility structs and functions.

#[macro_use]
mod errors;

mod callback;
mod event_listener;

use std::time::Duration;

use derive_more::{Add, From, Sub};
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
use std::{
    ops::Mul,
    rc::{Rc, Weak},
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

/// Wrapper around [`Duration`] which can be transformed into i32 for JS side
/// timers.
///
/// Also [`JsDuration`] can be multiplied by [`f32`].
#[derive(Debug, From, Copy, Clone, Add, Sub, PartialEq, Eq, PartialOrd, Ord)]
pub struct JsDuration(Duration);

impl JsDuration {
    /// Converts this [`JsDuration`] into `i32` milliseconds.
    pub fn into_js_duration(self) -> i32 {
        self.0.as_millis() as i32
    }
}

impl Mul<u32> for JsDuration {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Mul<f32> for JsDuration {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(Duration::from_millis(
            (self.0.as_millis() as f32 * rhs) as u64,
        ))
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

/// Trait which adds ability to upgrade [`Weak`]'s of Jason handlers
/// and returns [`HandlerDetachedError`] transformed into something which
/// can be transformed [`From`] [`JasonError`] if handler considered as
/// detached.
pub trait JasonWeakHandler<I> {
    /// Tries to upgrade handler and if it considered as detached - returns
    /// [`HandlerDetachedError`] transformed into required error.
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
