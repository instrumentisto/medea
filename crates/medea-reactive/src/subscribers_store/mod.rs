//! Stores for updates subscribers.

pub mod common;
pub mod progressable;

use futures::stream::LocalBoxStream;

/// Store for updates subscribers.
pub trait SubscribersStore<T, O>: Default {
    /// Sends data update to the all subscribers.
    fn send_update(&self, value: T);

    /// Creates new updates subscription.
    ///
    /// Returns [`Stream`] yielding elements sent with
    /// [`SubscribersStore::send_update()`] calls.
    ///
    /// [`Stream`]: futures::Stream
    fn subscribe(&self) -> LocalBoxStream<'static, O>;

    /// Wraps the provided `value` to the output type.
    #[must_use]
    fn wrap(&self, value: T) -> O;
}
