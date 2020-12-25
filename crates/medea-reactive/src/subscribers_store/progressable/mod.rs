//! Progressable [`SubscribersStore`].

pub mod guarded;

use std::{cell::RefCell, rc::Rc};

use futures::{
    channel::mpsc, future::LocalBoxFuture, stream::LocalBoxStream, Future,
};

use crate::{subscribers_store::SubscribersStore, ObservableCell};

pub use self::guarded::{Guard, Guarded};
use futures::FutureExt as _;
use std::{
    fmt,
    fmt::Formatter,
    ops::Deref,
    pin::Pin,
    task::{Context, Poll},
};

pub trait RecheckableFutureExt: Future + Unpin {
    fn is_done(&self) -> bool;

    fn refresh(&mut self);
}

impl<F: ?Sized + RecheckableFutureExt> RecheckableFutureExt for Box<F> {
    fn is_done(&self) -> bool {
        <F as RecheckableFutureExt>::is_done(&*self)
    }

    fn refresh(&mut self) {
        <F as RecheckableFutureExt>::refresh(&mut *self)
    }
}

pub struct RecheckableCounterFuture {
    counter: Rc<ObservableCell<u32>>,
    pending_fut: LocalBoxFuture<'static, ()>,
}

impl RecheckableFutureExt for RecheckableCounterFuture {
    fn is_done(&self) -> bool {
        self.counter.get() == 0
    }

    fn refresh(&mut self) {
        self.pending_fut = Box::pin(self.counter.when_eq(0).map(|_| ()));
    }
}

impl fmt::Debug for RecheckableCounterFuture {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecheckableCounterFuture")
            .field("counter", &self.counter)
            .finish()
    }
}

impl RecheckableCounterFuture {
    fn new(counter: Rc<ObservableCell<u32>>) -> Self {
        Self {
            pending_fut: Box::pin(counter.when_eq(0).map(|_| ())),
            counter,
        }
    }
}

impl Future for RecheckableCounterFuture {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        self.pending_fut.as_mut().poll(cx)
    }
}

#[derive(Debug)]
pub struct JoinRecheckableCounterFuture<F> {
    pending: Vec<F>,
    done: Vec<F>,
}

impl<F> JoinRecheckableCounterFuture<F> {
    fn new(pending: Vec<F>) -> Self {
        Self {
            pending,
            done: Vec::new(),
        }
    }
}

impl<F: RecheckableFutureExt> RecheckableFutureExt
    for JoinRecheckableCounterFuture<F>
{
    fn is_done(&self) -> bool {
        !self.done.iter().any(|f| !f.is_done())
    }

    fn refresh(&mut self) {
        let mut i = 0;
        while i != self.done.len() {
            if !self.done[i].is_done() {
                let mut pending = self.done.remove(i);
                pending.refresh();
                self.pending.push(pending);
            } else {
                i += 1;
            }
        }
    }
}

impl<F: RecheckableFutureExt> Future for JoinRecheckableCounterFuture<F> {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let mut i = 0;
        while i != self.pending.len() {
            match Pin::new(&mut self.pending[0]).as_mut().poll(cx) {
                Poll::Ready(_) => {
                    let done = self.pending.remove(i);
                    self.done.push(done);
                }
                Poll::Pending => {
                    i += 1;
                }
            }
        }

        if self.pending.is_empty() {
            let mut is_ready = true;
            let mut i = 0;
            while i != self.done.len() {
                if self.done[i].is_done() {
                    i += 1;
                } else {
                    let mut pending = self.done.remove(i);
                    pending.refresh();
                    self.pending.push(pending);
                    is_ready = false;
                }
            }

            if is_ready {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        } else {
            Poll::Pending
        }
    }
}

pub fn join_all<F: RecheckableFutureExt>(
    futs: Vec<F>,
) -> JoinRecheckableCounterFuture<F> {
    JoinRecheckableCounterFuture::new(futs)
}

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
