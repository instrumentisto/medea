//! Implementation of the stores for the updates subscribers.

pub(crate) mod common;
pub(crate) mod progressable;

use futures::stream::LocalBoxStream;

/// Store for the updates subscribers.
pub trait SubscribersStore<T, O>: Default {
    /// Sends data update to the all subscribers.
    fn send_update(&self, value: T);

    /// Creates new updates subscription.
    ///
    /// Returns [`Stream`] that will yield elements sent with
    /// [`SubscribersStore::send_update`] calls.
    ///
    /// [`Stream`]: futures::stream::Stream
    fn subscribe(&self) -> LocalBoxStream<'static, O>;

    /// Wraps provided value to output type.
    fn wrap(&self, value: T) -> O;
}
