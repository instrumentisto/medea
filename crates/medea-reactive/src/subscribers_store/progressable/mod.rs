//! Progressable [`SubscribersStore`].

pub mod guarded;
pub mod recheckable_future;

use std::{
    cell::RefCell,
    fmt,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use futures::{
    channel::mpsc, future::LocalBoxFuture, stream::LocalBoxStream, Future,
    FutureExt,
};

use crate::{subscribers_store::SubscribersStore, ObservableCell};

pub use self::{
    guarded::{Guard, Guarded},
    recheckable_future::RecheckableFutureExt,
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
    pub fn when_all_processed(&self) -> RecheckableCounterFuture {
        RecheckableCounterFuture::new(Rc::clone(&self.counter))
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

/// [`RecheckableFutureExt`] for [`SubStore::subscribe`].
pub struct RecheckableCounterFuture {
    /// Reference to the [`SubStore::counter`].
    counter: Rc<ObservableCell<u32>>,

    /// Current [`Future`] which will be polled on
    /// [`RecheckableCounterFuture::poll`].
    pending_fut: Option<LocalBoxFuture<'static, ()>>,
}

impl RecheckableFutureExt for RecheckableCounterFuture {
    /// Returns `true` if [`RecheckableCounterFuture::counter`] is `0`.
    fn is_done(&self) -> bool {
        self.counter.get() == 0
    }

    /// Refreshes [`RecheckableCounterFuture::pending_fut`] with a new
    /// [`RecheckableCounterFuture::counter`]'s [`ObservableCell::when_eq`]
    /// [`Future`].
    fn restart(&mut self) {
        self.pending_fut = Some(Box::pin(self.counter.when_eq(0).map(|_| ())));
    }
}

impl fmt::Debug for RecheckableCounterFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecheckableCounterFuture")
            .field("counter", &self.counter)
            .finish()
    }
}

impl RecheckableCounterFuture {
    /// Returns new [`RecheckableCounterFuture`] for the provided counter.
    pub(super) fn new(counter: Rc<ObservableCell<u32>>) -> Self {
        Self {
            pending_fut: None,
            counter,
        }
    }
}

impl Future for RecheckableCounterFuture {
    type Output = ();

    #[allow(clippy::option_if_let_else)]
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        if let Some(fut) = self.pending_fut.as_mut() {
            fut.as_mut().poll(cx)
        } else {
            self.restart();
            self.poll(cx)
        }
    }
}
