//! Miscellaneous utility structs and functions.

#[macro_use]
mod errors;

mod callback;
mod event_listener;
mod resettable_delay;

use std::{convert::TryInto as _, ops::Mul, time::Duration};

use derive_more::{From, Sub};
use futures::future::AbortHandle;
use js_sys::{Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Window;

#[cfg(debug_assertions)]
pub use self::errors::console_error;
#[doc(inline)]
pub use self::{
    callback::{Callback0, Callback1, Callback2},
    errors::{
        HandlerDetachedError, JasonError, JsCaused, JsError, JsonParseError,
    },
    event_listener::{EventListener, EventListenerBindError},
    resettable_delay::{resettable_delay_for, ResettableDelayHandle},
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
#[derive(Clone, Copy, Debug, From, PartialEq, PartialOrd, Sub)]
pub struct JsDuration(Duration);

impl JsDuration {
    /// Converts this [`JsDuration`] into `i32` milliseconds.
    ///
    /// Unfortunately, [`web_sys`] believes that only `i32` can be passed to a
    /// `setTimeout`. But it is unlikely we will need a duration of more,
    /// than 596 hours, so it was decided to simply truncate the number. If we
    /// will need a longer duration in the future, then we can implement this
    /// with a few `setTimeout`s.
    #[inline]
    pub fn into_js_duration(self) -> i32 {
        self.0.as_millis().try_into().unwrap_or(i32::max_value())
    }
}

impl Mul<u32> for JsDuration {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: u32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Mul<f32> for JsDuration {
    type Output = Self;

    #[inline]
    fn mul(self, mut rhs: f32) -> Self::Output {
        // Emulation of JS side's 'setTimeout' behavior which will be instantly
        // resolved if call it with negative number.
        if rhs < 0.0 {
            rhs = 0.0;
        };
        Self(self.0.mul_f64(rhs.into()))
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

/// Upgrades provided [`Weak`] reference, mapping it to a [`Result`] with
/// [`HandlerDetachedError`] and invokes [`Into::into`] on the error.
/// If the errot type cannot be inferred, then you can provide a concrete type
/// (usually being [`JasonError`] or [`JsValue`]).
///
/// [`Weak`]: std::rc::Weak
macro_rules! upgrade_or_detached {
    ($v:expr) => {{
        $v.upgrade()
            .ok_or_else(|| new_js_error!(HandlerDetachedError))
    }};
    ($v:expr, $err:ty) => {{
        $v.upgrade()
            .ok_or_else(|| new_js_error!(HandlerDetachedError => $err))
    }};
}

/// Adds [`tracerr`] information to the provided error, wraps it into
/// [`JasonError`] and converts it into the expected error type.
///
/// This macro has two syntaxes:
/// - `new_js_error!(DetachedStateError)` - converts provided error wrapped into
///   [`JasonError`] with [`Into::into`] automatically;
/// - `new_js_error!(DetachedStateError => JsError)` - annotates explicitly
///   which type conversion is required.
macro_rules! new_js_error {
    ($e:expr) => {
        $crate::utils::JasonError::from(&tracerr::new!($e)).into()
    };
    ($e:expr => $o:ty) => {
        <$o>::from($crate::utils::JasonError::from(&tracerr::new!($e)))
    };
}

/// Creates new [`HashMap`] from a list of key-value pairs.
///
/// # Example
///
/// ```rust
/// # use medea_jason::hashmap;
/// let map = hashmap! {
///     "a" => 1,
///     "b" => 2,
/// };
/// assert_eq!(map["a"], 1);
/// assert_eq!(map["b"], 2);
/// assert_eq!(map.get("c"), None);
/// ```
///
/// [`HashMap`]: std::collections::HashMap
#[macro_export]
macro_rules! hashmap {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(hashmap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { hashmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let _cap = hashmap!(@count $($key),*);
            let mut _map = ::std::collections::HashMap::with_capacity(_cap);
            $(
                let _ = _map.insert($key, $value);
            )*
            _map
        }
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

/// [`Future`] which resolves after the provided [`JsDuration`].
///
/// [`Future`]: std::future::Future
pub async fn delay_for(delay_ms: JsDuration) {
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

/// Wrapper around [`AbortHandle`] which aborts [`Future`] on [`Drop`].
#[derive(Debug, From)]
pub struct TaskHandle(AbortHandle);

impl Drop for TaskHandle {
    fn drop(&mut self) {
        self.0.abort();
    }
}
