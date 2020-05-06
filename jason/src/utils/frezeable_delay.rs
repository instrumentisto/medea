//! Implementation of the timer which can be freezed and unfreezed.
//!
//! # Usage
//!
//! ```ignore
//! # use std::{
//! #    time::Duration,
//! #    thread::sleep,
//! # };
//! # use medea_jason::utils::{FreezeableDelayHandle, freezeable_delay_for};
//! # use wasm_bindgen_futures::spawn_local;
//! let (timeout, handle) = freezeable_delay_for(Duration::from_secs(3));
//! spawn_local(async move {
//!     timeout.await;
//!     println!("Timer Future was finally resolved after 6 seconds!");
//! });
//!
//! handle.freeze();
//! sleep(Duration::from_secs(3));
//! handle.unfreeze();
//! ```

use std::{cell::RefCell, future::Future, rc::Rc, time::Duration};

use futures::{channel::oneshot, future, future::AbortHandle};
use wasm_bindgen_futures::spawn_local;

use crate::utils::delay_for;

type FutureResolver = Rc<RefCell<Option<oneshot::Sender<()>>>>;

/// Returns [`Future`] which will be resolved after provided [`Duration`] (but
/// this time can be freezed) and [`FreezeableDelayHandle`] with which you can
/// freeze or unfreeze returned [`Future`] timer.
pub fn freezeable_delay_for(
    delay_ms: Duration,
) -> (impl Future<Output = ()>, FreezeableDelayHandle) {
    FreezeableDelayHandle::new_delay(delay_ms)
}

/// Timer which can be stopped and started again with reseted [`Duration`].
pub struct FreezeableDelayHandle {
    /// [`oneshot::Sender`] with which timer [`Future`] can be resolved.
    ///
    /// If it `None` then timer [`Future`] was already resolved.
    future_resolver: FutureResolver,

    /// [`Duration`] after which timer [`Future`] should be resolved.
    timeout: Duration,

    /// [`AbortHandle`] with which you can freeze timer [`Future`].
    abort_handle: RefCell<AbortHandle>,
}

impl FreezeableDelayHandle {
    /// Returns [`Future`] which will be resolved after provided [`Duration`]
    /// (but this time can be freezed) and [`FreezeableDelayHandle`] with
    /// which you can freeze or unfreeze returned [`Future`] timer.
    fn new_delay(timeout: Duration) -> (impl Future<Output = ()>, Self) {
        let (tx, rx) = oneshot::channel();
        let tx = Rc::new(RefCell::new(Some(tx)));

        let (abort, _) = AbortHandle::new_pair();
        let this = Self {
            future_resolver: tx,
            abort_handle: RefCell::new(abort),
            timeout,
        };
        this.spawn_timer();

        (
            async move {
                let _ = rx.await;
            },
            this,
        )
    }

    /// Spawns new [`Future`] which will wait [`FreezeableDelayHandle::timeout`]
    /// and resolve timer [`Future`].
    ///
    /// Replaces [`FreezeableDelayHandle::abort_handle`] with newly spawned
    /// [`Future`]'s [`AbortHandle`].
    fn spawn_timer(&self) {
        let future_resolver = self.future_resolver.clone();
        let timeout = self.timeout;
        let (fut, abort) = future::abortable(async move {
            delay_for(timeout.into()).await;
            if let Some(future_resolver) = future_resolver.borrow_mut().take() {
                let _ = future_resolver.send(());
            }
        });
        spawn_local(async move {
            let _ = fut.await;
        });

        self.abort_handle.replace(abort);
    }

    /// Freezes this timer [`Future`] until [`FreezeableDelayHandle::unfreeze`]
    /// will be called.
    ///
    /// After [`FreezeableDelayHandle::unfreeze`] will be called, countdown of
    /// the timer [`Future`] will be started from the beginning.
    pub fn freeze(&self) {
        self.abort_handle.borrow().abort();
    }

    /// Unfreezes this timer [`Future`] and start countdown from the beginning.
    pub fn unfreeze(&self) {
        self.abort_handle.borrow().abort();
        self.spawn_timer();
    }
}
