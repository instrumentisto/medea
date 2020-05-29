//! Delay that can be stopped and started over again.

use std::{cell::RefCell, future::Future, rc::Rc, time::Duration};

use futures::{
    channel::oneshot,
    future,
    future::{AbortHandle, FutureExt},
};
use wasm_bindgen_futures::spawn_local;

use crate::utils::delay_for;

type FutureResolver = Rc<RefCell<Option<oneshot::Sender<()>>>>;

/// Returns [`Future`] that will be resolved after provided [`Duration`] and
/// [`ResettableDelayHandle`] that allows you to control delay future.
pub fn resettable_delay_for(
    delay: Duration,
) -> (impl Future<Output = ()>, ResettableDelayHandle) {
    ResettableDelayHandle::new(delay)
}

/// Handler to delay which can be stopped and started over again [`Duration`].
pub struct ResettableDelayHandle {
    /// [`oneshot::Sender`] with which timer [`Future`] can be resolved.
    ///
    /// If it `None` then timer [`Future`] was already resolved.
    future_resolver: FutureResolver,

    /// [`Duration`] after which delay will be resolved.
    timeout: Duration,

    /// [`AbortHandle`] with which you can stop delay completion timer.
    abort_handle: RefCell<AbortHandle>,
}

impl ResettableDelayHandle {
    /// Stops delay [`Future`] so it will never be resolved, if it haven't been
    /// resolved already (doest nothing in this case).
    pub fn stop(&self) {
        self.abort_handle.borrow().abort();
    }

    /// Resets delay [`Future`] timer, starting countdown from the beginning.
    pub fn reset(&self) {
        self.abort_handle.borrow().abort();
        self.spawn_timer();
    }

    /// Creates delay [`Future`] and its [`ResettableDelayHandle`], schedules
    /// delay future completion.
    fn new(timeout: Duration) -> (impl Future<Output = ()>, Self) {
        let (tx, rx) = oneshot::channel();
        let tx = Rc::new(RefCell::new(Some(tx)));

        let (abort, _) = AbortHandle::new_pair();
        let this = Self {
            future_resolver: tx,
            abort_handle: RefCell::new(abort),
            timeout,
        };
        this.spawn_timer();

        let delay_fut = async move {
            if rx.await.is_err() {
                // delay was stopped and handle was dropped
                future::pending::<()>().await;
            };
        };

        (delay_fut, this)
    }

    /// Spawns timer, that will resolve delay [`Future`].
    fn spawn_timer(&self) {
        let future_resolver = self.future_resolver.clone();
        let timeout = self.timeout;
        let (fut, abort) = future::abortable(async move {
            delay_for(timeout.into()).await;
            if let Some(rslvr) = future_resolver.borrow_mut().take() {
                let _ = rslvr.send(());
            }
        });
        spawn_local(fut.map(|_| ()));

        self.abort_handle.replace(abort);
    }
}
