pub(crate) mod common;
pub(crate) mod progressable;

use futures::stream::LocalBoxStream;

pub trait SubscribersStore<T, O>: Default {
    fn send_update(&self, value: T);

    fn new_subscription(
        &self,
        initial_values: Vec<T>,
    ) -> LocalBoxStream<'static, O>;
}
