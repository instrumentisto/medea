//! Implementation of the stores for the updates subscribers.

pub(crate) mod common;
pub(crate) mod progressable;

use futures::stream::LocalBoxStream;

/// Store for the updates subscribers.
pub trait SubscribersStore<T, O>: Default {
    /// Sends data update to the all subscribers.
    fn send_update(&self, value: T);

    /// Returns [`Stream`] into which all sent with
    /// [`SubscribersStore::send_update`] updates will be sent.
    ///
    /// [`Stream`]: futures::stream::Stream
    fn new_subscription(
        &self,
        initial_values: Vec<T>,
    ) -> LocalBoxStream<'static, O>;
}
