//! Miscellaneous utility structs and functions.

#[macro_use]
mod errors;

pub mod component;
mod resettable_delay;

use std::future::Future;

use derive_more::From;
use futures::future::{self, AbortHandle};
use medea_reactive::Guarded;

#[doc(inline)]
pub use self::{
    component::{AsProtoState, Component, SynchronizableState, Updatable},
    errors::{Caused, JsonParseError},
    resettable_delay::{resettable_delay_for, ResettableDelayHandle},
};

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
