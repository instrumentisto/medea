//! Local session description wrapper.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use futures::{
    future,
    future::{Either, LocalBoxFuture},
    stream::LocalBoxStream,
    StreamExt as _,
};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::utils::{resettable_delay_for, ResettableDelayHandle};

const DESCRIPTION_APPROVE_TIMEOUT: Duration = Duration::from_secs(10);

/// Local session description wrapper.
///
/// Stores current and previous descriptions and may rollback to the previous
/// one if new description won't be approved in a configured timeout.
#[derive(Clone, Debug, Default)]
pub struct LocalSdp(Rc<Inner>);

impl LocalSdp {
    /// Returns new empty [`LocalSdp`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns [`LocalBoxStream`] into which all current SDP offer updates will
    /// be sent.
    #[inline]
    pub fn subscribe(&self) -> LocalBoxStream<'static, Option<String>> {
        self.0.current_sdp.subscribe()
    }

    /// Returns [`Future`] that will be resolved when current SDP offer will be
    /// approved by Media Server.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_approved(&self) -> LocalBoxFuture<'static, ()> {
        let approved = Rc::clone(&self.0.approved);
        Box::pin(async move {
            let _ = approved.when_eq(true).await;
        })
    }

    /// Returns [`Stream`] into which `()` will be sent on every SDP offer
    /// approve.
    ///
    /// [`Stream`]: futures::stream::Stream
    #[inline]
    pub fn on_approve(&self) -> LocalBoxStream<'static, ()> {
        Box::pin(self.0.approved.subscribe().filter_map(|approved| {
            future::ready(if approved { Some(()) } else { None })
        }))
    }

    /// Rollbacks [`LocalSdp`] to the previous one.
    #[inline]
    pub fn rollback(&self) {
        self.0.current_sdp.set(self.0.prev_sdp.borrow().clone());
        self.0.approved.set(true);
    }

    /// Sets the provided SDP as the current one, marks it as unapproved and
    /// schedules task to wait for a SDP approval.
    pub fn unapproved_set(&self, sdp: String) {
        let prev_sdp = self.0.current_sdp.replace(Some(sdp));
        self.0.prev_sdp.replace(prev_sdp);
        self.0.approved.set(false);
        self.spawn_rollback_task();
    }

    /// Approves the current [`LocalSdp`] offer.
    pub fn approved_set(&self, sdp: String) {
        let is_current_approved =
            self.0.current_sdp.borrow().as_ref() == Some(&sdp);

        if !is_current_approved {
            let is_restart_needed = self
                .0
                .prev_sdp
                .borrow()
                .as_ref()
                .map(|prev| prev == &sdp)
                .unwrap_or_default();
            if is_restart_needed {
                self.0.restart_needed.set(true);
            }
            self.0.current_sdp.replace(Some(sdp));
        }
        self.0.approved.set(true);
    }

    /// Returns the current SDP offer.
    #[inline]
    #[must_use]
    pub fn current(&self) -> Option<String> {
        self.0.current_sdp.get()
    }

    /// Indicates whether current [`LocalSdp`] state is rollback, meaning that
    /// the current SDP equals to the previous SDP.
    #[must_use]
    pub fn is_rollback(&self) -> bool {
        self.0
            .current_sdp
            .borrow()
            .as_ref()
            .map_or(false, |current| {
                self.0
                    .prev_sdp
                    .borrow()
                    .as_ref()
                    .map(|prev| prev == current)
                    .unwrap_or_default()
            })
    }

    /// Stops the current SDP rollback task countdown, if any.
    #[inline]
    pub fn stop_timeout(&self) {
        self.0.is_rollback_timeout_stopped.set(true);
        if let Some(handle) = self.0.rollback_task_handle.borrow().as_ref() {
            handle.stop();
        }
    }

    /// Resets the current SDP rollback task countdown, if any.
    #[inline]
    pub fn resume_timeout(&self) {
        self.0.is_rollback_timeout_stopped.set(false);
        if let Some(handle) = self.0.rollback_task_handle.borrow().as_ref() {
            handle.reset();
        }
    }

    /// Spawns task that will call [`LocalSdp::rollback()`] if the current SDP
    /// won't be approved in [`DESCRIPTION_APPROVE_TIMEOUT`].
    fn spawn_rollback_task(&self) {
        let (timeout, rollback_task) = resettable_delay_for(
            DESCRIPTION_APPROVE_TIMEOUT,
            self.0.is_rollback_timeout_stopped.get(),
        );
        spawn_local({
            let this = self.clone();
            async move {
                if let Either::Right(_) =
                    future::select(this.when_approved(), Box::pin(timeout))
                        .await
                {
                    this.rollback();
                };
            }
        });

        self.0.rollback_task_handle.replace(Some(rollback_task));
    }

    /// Indicates whether a new SDP offer is needed after rollback's completion.
    #[inline]
    #[must_use]
    pub fn is_restart_needed(&self) -> bool {
        self.0.restart_needed.get()
    }
}

#[derive(Debug)]
struct Inner {
    /// Currently applied session description.
    current_sdp: ObservableCell<Option<String>>,

    /// Previously applied session description.
    prev_sdp: RefCell<Option<String>>,

    /// Flag which indicates that Media Server approved this SDP
    /// [`Inner::current_sdp`].
    ///
    /// On every SDP offer update this field should be reset to `false` and
    /// if this field doesn't transits into `true` within
    /// [`DESCRIPTION_APPROVE_TIMEOUT`], then [`Inner::current_sdp`] should be
    /// rolled back to the [`Inner::prev_sdp`].
    approved: Rc<ObservableCell<bool>>,

    /// Timeout of the [`Inner::approved`] transition.
    rollback_task_handle: RefCell<Option<ResettableDelayHandle>>,

    /// Indicator whether [`Inner::rollback_task_handle`] timeout is stopped.
    is_rollback_timeout_stopped: Cell<bool>,

    /// Indicator whether negotiation restart is needed.
    restart_needed: Cell<bool>,
}

impl Default for Inner {
    #[inline]
    fn default() -> Self {
        Self {
            prev_sdp: RefCell::new(None),
            current_sdp: ObservableCell::new(None),
            approved: Rc::new(ObservableCell::new(true)),
            rollback_task_handle: RefCell::new(None),
            is_rollback_timeout_stopped: Cell::new(false),
            restart_needed: Cell::new(false),
        }
    }
}
