//! [`Task`] for execution by a [`platform::dart::executor`].

use std::rc::Rc;

use std::{
    cell::RefCell,
    mem::ManuallyDrop,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use futures::future::LocalBoxFuture;

use crate::platform::dart::executor::task_wake;

/// Inner [`Task`]'s data.
struct Inner {
    /// An actual [`Future`] that this [`Task`] is driving.
    future: LocalBoxFuture<'static, ()>,

    /// Handle for waking up this [`Task`].
    waker: Waker,
}

/// Wrapper for a [`Future`] that can be polled by an external single threaded
/// Dart executor.
pub struct Task {
    /// [`Task`]'s inner data containing an actual [`Future`] and its
    /// [`Waker`]. Dropped on the [`Task`] completion.
    inner: RefCell<Option<Inner>>,
}

impl Task {
    /// Creates a new [`Task`] out of the given [`Future`].
    #[must_use]
    pub fn new(future: LocalBoxFuture<'static, ()>) -> Rc<Self> {
        let this = Rc::new(Self {
            inner: RefCell::new(None),
        });

        let waker =
            unsafe { Waker::from_raw(Task::into_raw_waker(Rc::clone(&this))) };
        this.inner.borrow_mut().replace(Inner { future, waker });

        this
    }

    /// Polls the underlying [`Future`].
    ///
    /// Polling after [`Future`]'s completion is no-op.
    pub fn poll(&self) -> Poll<()> {
        let mut borrow = self.inner.borrow_mut();

        // Just ignore poll request if the `Future` is completed.
        let inner = match borrow.as_mut() {
            Some(inner) => inner,
            None => return Poll::Ready(()),
        };

        let poll = {
            let mut cx = Context::from_waker(&inner.waker);
            inner.future.as_mut().poll(&mut cx)
        };

        // Cleanup resources if future is ready.
        if poll.is_ready() {
            *borrow = None;
        }

        poll
    }

    /// Calls the [`task_wake()`] function by the provided reference.
    fn wake_by_ref(this: &Rc<Self>) {
        task_wake(Rc::as_ptr(this));
    }

    /// Pretty much a copy of [`std::task::Wake`] implementation but for
    /// `Rc<?Send + ?Sync>` instead of `Arc<Send + Sync>` since we are sure
    /// that everything will run on a single thread.
    #[inline(always)]
    fn into_raw_waker(this: Rc<Self>) -> RawWaker {
        // Refer to `RawWakerVTable::new()` documentation for better
        // understanding of what following functions do.
        unsafe fn raw_clone(ptr: *const ()) -> RawWaker {
            let ptr = ManuallyDrop::new(Rc::from_raw(ptr.cast::<Task>()));
            Task::into_raw_waker(Rc::clone(&(*ptr)))
        }

        unsafe fn raw_wake(ptr: *const ()) {
            let ptr = Rc::from_raw(ptr.cast::<Task>());
            Task::wake_by_ref(&ptr);
        }

        unsafe fn raw_wake_by_ref(ptr: *const ()) {
            let ptr = ManuallyDrop::new(Rc::from_raw(ptr.cast::<Task>()));
            Task::wake_by_ref(&ptr);
        }

        unsafe fn raw_drop(ptr: *const ()) {
            drop(Rc::from_raw(ptr.cast::<Task>()));
        }

        const VTABLE: RawWakerVTable =
            RawWakerVTable::new(raw_clone, raw_wake, raw_wake_by_ref, raw_drop);

        RawWaker::new(Rc::into_raw(this).cast::<()>(), &VTABLE)
    }
}
