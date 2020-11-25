use std::cell::RefCell;

use futures::{channel::mpsc, stream::LocalBoxStream};

use super::SubscribersStore;

#[derive(Debug)]
pub struct BasicSubStore<T>(RefCell<Vec<mpsc::UnboundedSender<T>>>);

impl<T> Default for BasicSubStore<T> {
    fn default() -> Self {
        Self(RefCell::new(Vec::new()))
    }
}

impl<T> SubscribersStore<T, T> for BasicSubStore<T>
where
    T: Clone + 'static,
{
    fn send_update(&self, value: T) {
        self.0
            .borrow_mut()
            .retain(|sub| sub.unbounded_send(value.clone()).is_ok());
    }

    fn new_subscription(
        &self,
        initial_values: Vec<T>,
    ) -> LocalBoxStream<'static, T> {
        let (tx, rx) = mpsc::unbounded();
        initial_values.into_iter().for_each(|value| {
            let _ = tx.unbounded_send(value);
        });

        Box::pin(rx)
    }
}
