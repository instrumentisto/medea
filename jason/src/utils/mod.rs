//! Miscellaneous utility structs and functions.

#[macro_use]
mod errors;

mod callback;
mod event_listener;

use std::{
    ops::Mul,
    rc::{Rc, Weak},
    time::Duration,
};

use bigdecimal::{BigDecimal, ToPrimitive as _};
use derive_more::{Add, Div, From, Sub};
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

/// Wrapper around [`Duration`] which can be transformed into [`i32`] for JS
/// side timers.
///
/// Also [`JsDuration`] can be multiplied by [`f32`].
#[derive(
    Debug, From, Copy, Clone, Add, Sub, PartialEq, Eq, PartialOrd, Ord, Div,
)]
pub struct JsDuration(Duration);

impl JsDuration {
    /// Converts this [`JsDuration`] into `i32` milliseconds.
    // Unfortunately, 'web_sys' believes that only 'i32' can be passed to a
    // 'setTimeout'. But it is unlikely we will need a duration of more,
    // than 596 hours, so it was decided to simply truncate the number. If we
    // will need a longer duration in the future, then we can implement this
    // with a few 'setTimeouts'.
    #[allow(clippy::cast_possible_truncation)]
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

    // Truncation here is normal, because we will still be limited in
    // 'JsDuration::into_js_duration' to the 596 hours (which is less than
    // 'u64' limit).
    #[allow(clippy::cast_possible_truncation)]
    fn mul(self, rhs: f32) -> Self::Output {
        // Always positive.
        let duration_ms = BigDecimal::from(self.0.as_millis() as u64);
        // Can be negative, but that will be fixed in the result of calculation
        // which will be transformed to 'u128' bellow.
        let multiplier = BigDecimal::from(rhs);
        // Theoretically we can get negative number here. But all negative
        // numbers will be reduced to zero. This is default behavior of the
        // JavaScript's 'setTimeout' and it's OK here.
        //
        // We can get 'Err' here only if value is less than zero (this can be
        // proved by looking at the source code).
        let multiplied_duration =
            (duration_ms * multiplier).to_u64().unwrap_or(0);
        Self(Duration::from_millis(multiplied_duration))
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
        self.upgrade().ok_or_else(|| {
            JasonError::from(tracerr::new!(HandlerDetachedError)).into()
        })
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

/// [`Future`] which resolves after provided [`JsDuration`].
pub async fn resolve_after(delay_ms: JsDuration) {
    JsFuture::from(Promise::new(&mut |yes, _| {
        window()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &yes,
                delay_ms.into_js_duration(),
            )
            .unwrap();
    }))
    .await
    .unwrap();
}
