//! Progressable [`SubscribersStore`].

pub mod guarded;
pub mod processed;

use std::{cell::RefCell, rc::Rc};

use futures::{channel::mpsc, stream::LocalBoxStream};

use crate::{subscribers_store::SubscribersStore, ObservableCell};

pub use self::{
    guarded::{Guard, Guarded},
    processed::{AllProcessed, Processed},
};

/// [`SubscribersStore`] for progressable collections/field.
///
/// Will provided [`Guarded`] with an updated data to the
/// [`SubscribersStore::subscribe()`] [`Stream`].
///
/// You can wait for updates processing with a
/// [`SubStore::when_all_processed()`] method.
///
/// [`Stream`]: futures::Stream
#[derive(Debug)]
pub struct SubStore<T> {
    /// All subscribers of this store.
    store: RefCell<Vec<mpsc::UnboundedSender<Guarded<T>>>>,

    /// Manager recognizing when all sent updates are processed.
    counter: Rc<ObservableCell<u32>>,
}

impl<T> Default for SubStore<T> {
    #[inline]
    fn default() -> Self {
        Self {
            store: RefCell::new(Vec::new()),
            counter: Rc::new(ObservableCell::new(0)),
        }
    }
}

impl<T> SubStore<T> {
    /// Returns [`Future`] resolving when all subscribers processes update.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_all_processed(&self) -> Processed<'static> {
        let counter = Rc::clone(&self.counter);
        Processed::new(Box::new(move || {
            let counter = Rc::clone(&counter);
            Box::pin(async move {
                let _ = counter.when_eq(0).await;
            })
        }))
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

    #[inline]
    fn wrap(&self, value: T) -> Guarded<T> {
        Guarded::wrap(value, Rc::clone(&self.counter))
    }
}
