//! Implementation of the progressable [`SubscribersStore`].

mod manager;
mod value;

use std::cell::RefCell;

use futures::{channel::mpsc, future::LocalBoxFuture, stream::LocalBoxStream};

use crate::subscribers_store::SubscribersStore;

use self::manager::Manager;

pub use self::value::Value;

/// [`SubscribersStore`] for progressable collections/field.
///
/// Will provided [`ProgressableValue`] with updated data to the
/// [`SubscribersStore::new_subscription`] [`Stream`].
///
/// You can wait for updates processing with a [`SubStore::when_all_processed`]
/// method.
///
/// [`Stream`]: futures::stream::Stream
#[derive(Debug)]
pub struct SubStore<T> {
    /// All subscribers of this store.
    store: RefCell<Vec<mpsc::UnboundedSender<Value<T>>>>,

    /// Manager which will recognise when all sent updates are processed.
    manager: Manager,
}

impl<T> Default for SubStore<T> {
    fn default() -> Self {
        Self {
            store: RefCell::new(Vec::new()),
            manager: Manager::new(),
        }
    }
}

impl<T> SubStore<T> {
    /// Returns [`Future`] which will be resolved when all subscribers processes
    /// updates.
    pub fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        self.manager.when_all_processed()
    }
}

impl<T> SubscribersStore<T, Value<T>> for SubStore<T>
where
    T: Clone + 'static,
{
    fn send_update(&self, value: T) {
        self.store.borrow_mut().retain(|sub| {
            sub.unbounded_send(self.manager.new_value(value.clone()))
                .is_ok()
        });
    }

    fn new_subscription(
        &self,
        initial_values: Vec<T>,
    ) -> LocalBoxStream<'static, Value<T>> {
        let (tx, rx) = mpsc::unbounded();

        initial_values.into_iter().for_each(|value| {
            let _ = tx.unbounded_send(self.manager.new_value(value));
        });

        self.store.borrow_mut().push(tx);

        Box::pin(rx)
    }
}
