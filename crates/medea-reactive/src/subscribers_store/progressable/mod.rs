//! Implementation of the progressable [`SubscribersStore`].

mod guarded;

use std::{cell::RefCell, rc::Rc};

use futures::{channel::mpsc, future::LocalBoxFuture, stream::LocalBoxStream};

use crate::{subscribers_store::SubscribersStore, ObservableCell};

pub use self::guarded::{Guard, Guarded};

/// [`SubscribersStore`] for progressable collections/field.
///
/// Will provided [`Value`] with updated data to the
/// [`SubscribersStore::new_subscription`] [`Stream`].
///
/// You can wait for updates processing with a [`SubStore::when_all_processed`]
/// method.
///
/// [`Stream`]: futures::stream::Stream
#[derive(Debug)]
pub struct SubStore<T> {
    /// All subscribers of this store.
    store: RefCell<Vec<mpsc::UnboundedSender<Guarded<T>>>>,

    /// Manager which will recognise when all sent updates are processed.
    counter: Rc<ObservableCell<u32>>,
}

impl<T> Default for SubStore<T> {
    fn default() -> Self {
        Self {
            store: RefCell::new(Vec::new()),
            counter: Rc::new(ObservableCell::new(0)),
        }
    }
}

impl<T> SubStore<T> {
    /// Returns [`Future`] which will be resolved when all subscribers processes
    /// updates.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        let fut = self.counter.when_eq(0);
        Box::pin(async move {
            let _ = fut.await;
        })
    }
}

impl<T> SubscribersStore<T, Guarded<T>> for SubStore<T>
where
    T: Clone + 'static,
{
    fn send_update(&self, value: T) {
        self.store
            .borrow_mut()
            .retain(|sub| sub.unbounded_send(self.wrap(value.clone())).is_ok());
    }

    fn subscribe(&self) -> LocalBoxStream<'static, Guarded<T>> {
        let (tx, rx) = mpsc::unbounded();
        self.store.borrow_mut().push(tx);
        Box::pin(rx)
    }

    fn wrap(&self, value: T) -> Guarded<T> {
        Guarded::new(value, Rc::clone(&self.counter))
    }
}
