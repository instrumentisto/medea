mod manager;
mod value;

use std::cell::RefCell;

use futures::{channel::mpsc, future::LocalBoxFuture, stream::LocalBoxStream};

use crate::subscribers_store::SubscribersStore;

use self::manager::ProgressableManager;

pub use self::value::Value;

#[derive(Debug)]
pub struct SubStore<T> {
    store: RefCell<Vec<mpsc::UnboundedSender<Value<T>>>>,
    manager: ProgressableManager,
}

impl<T> Default for SubStore<T> {
    fn default() -> Self {
        Self {
            store: RefCell::new(Vec::new()),
            manager: ProgressableManager::new(),
        }
    }
}

impl<T> SubStore<T> {
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
            self.manager.incr_processors_count(1);
            let value = self.manager.new_value(value.clone());

            sub.unbounded_send(value).is_ok()
        });
    }

    fn new_subscription(
        &self,
        initial_values: Vec<T>,
    ) -> LocalBoxStream<'static, Value<T>> {
        let (tx, rx) = mpsc::unbounded();

        initial_values.into_iter().for_each(|value| {
            self.manager.incr_processors_count(1);
            let value = self.manager.new_value(value);
            let _ = tx.unbounded_send(value);
        });

        self.store.borrow_mut().push(tx);

        Box::pin(rx)
    }
}
