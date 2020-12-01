//! Implementation of the default [`SubscribersStore`] for collections.

use std::cell::RefCell;

use futures::{channel::mpsc, stream::LocalBoxStream};

use super::SubscribersStore;

/// Default [`SubscribersStore`] for collections.
#[derive(Debug)]
pub struct SubStore<T>(RefCell<Vec<mpsc::UnboundedSender<T>>>);

impl<T> Default for SubStore<T> {
    fn default() -> Self {
        Self(RefCell::new(Vec::new()))
    }
}

impl<T> SubscribersStore<T, T> for SubStore<T>
where
    T: Clone + 'static,
{
    fn send_update(&self, value: T) {
        self.0
            .borrow_mut()
            .retain(|sub| sub.unbounded_send(value.clone()).is_ok());
    }

    fn new_subscription(&self) -> LocalBoxStream<'static, T> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().push(tx);
        Box::pin(rx)
    }

    fn replay(&self, values: Vec<T>) -> LocalBoxStream<'static, T> {
        Box::pin(futures::stream::iter(values.into_iter()))
    }
}
