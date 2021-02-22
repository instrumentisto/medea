//! Miscellaneous utility structs and functions.

#[macro_use]
mod errors;

mod callback;
pub mod component;
mod resettable_delay;

use std::future::Future;

use derive_more::From;
use futures::future::{self, AbortHandle};
use medea_reactive::Guarded;

#[doc(inline)]
pub use self::{
    callback::{Callback0, Callback1},
    component::{AsProtoState, Component, SynchronizableState, Updatable},
    errors::{HandlerDetachedError, JasonError, JsCaused, JsonParseError},
    resettable_delay::{resettable_delay_for, ResettableDelayHandle},
};

/// Upgrades provided [`Weak`] reference, mapping it to a [`Result`] with
/// [`HandlerDetachedError`] and invokes [`Into::into`] on the error.
/// If the error type cannot be inferred, then you can provide a concrete type
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
/// - `new_js_error!(DetachedStateError => platform::Error)` - annotates
///   explicitly which type conversion is required.
macro_rules! new_js_error {
    ($e:expr) => {
        $crate::utils::JasonError::from(tracerr::new!($e)).into()
    };
    ($e:expr => $o:ty) => {
        <$o>::from($crate::utils::JasonError::from(tracerr::new!($e)))
    };
}

/// Wrapper around [`AbortHandle`] which aborts [`Future`] on [`Drop`].
///
/// [`Future`]: std::future::Future
#[derive(Debug, From)]
pub struct TaskHandle(AbortHandle);

impl Drop for TaskHandle {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Tries to upgrade [`Weak`] reference breaks cycle if upgrade fails.
macro_rules! upgrade_or_break {
    ($weak:tt) => {
        if let Some(this) = $weak.upgrade() {
            this
        } else {
            break;
        }
    };
}

/// Returns [`Future`] which will return the provided value being
/// [`Guarded::transpose()`]d.
///
/// Intended for use in [`StreamExt::filter_map()`].
///
/// [`StreamExt::filter_map()`]: futures::StreamExt::filter_map
#[inline]
pub fn transpose_guarded<T>(
    val: Guarded<Option<T>>,
) -> impl Future<Output = Option<Guarded<T>>> {
    future::ready(val.transpose())
}
