//! Local session description wrapper.

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{
    future,
    future::{Either, LocalBoxFuture},
    stream::LocalBoxStream,
};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::utils::{resettable_delay_for, ResettableDelayHandle};

const DESCRIPTION_APPROVE_TIMEOUT: Duration = Duration::from_secs(10);

/// Local session description wrapper.
///
/// Stores current and previous descriptions and may rollback to previous if new
/// description will not be approved in configured timeout.
#[derive(Clone, Debug, Default)]
pub struct LocalSdp(Rc<Inner>);

impl LocalSdp {
    /// Returns new empty [`LocalSdp`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns [`LocalBoxStream`] into which all current SDP offer updates will
    /// be sent.
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

    /// Rollbacks [`LocalSdp`] to the previous one.
    pub fn rollback(&self) {
        self.0.current_sdp.set(self.0.prev_sdp.borrow().clone());
        self.0.approved.set(true);
    }

    /// Sets provided SDP as current, marks it as unapproved and schedules task,
    /// that waits for SDP approval.
    pub fn unapproved_set(&self, sdp: String) {
        let prev_sdp = self.0.current_sdp.replace(Some(sdp));
        self.0.prev_sdp.replace(prev_sdp);
        self.0.approved.set(false);
        self.0
            .rollback_task_handle
            .replace(Some(self.spawn_rollback_task()));
    }

    /// Approves current [`LocalSdp`] offer.
    pub fn approved_set(&self, sdp: String) {
        let is_current_approved =
            self.0.current_sdp.borrow().as_ref() == Some(&sdp);

        if !is_current_approved {
            self.0.current_sdp.replace(Some(sdp));
        }
        self.0.approved.set(true);
    }

    /// Returns `true` if current [`LocalSdp`] state is rollback, meaning that
    /// current SDP equals previous SDP.
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

    /// Stops current SDP rollback task countdown (if any).
    pub fn stop_timeout(&self) {
        if let Some(handle) = self.0.rollback_task_handle.borrow().as_ref() {
            handle.stop();
        }
    }

    /// Resets current SDP rollback task countdown (if any).
    pub fn resume_timeout(&self) {
        if let Some(handle) = self.0.rollback_task_handle.borrow().as_ref() {
            handle.reset();
        }
    }

    /// Spawns task that will call [`LocalSdp::rollback()`] if current SDP won't
    /// be approved in [`DESCRIPTION_APPROVE_TIMEOUT`].
    fn spawn_rollback_task(&self) -> ResettableDelayHandle {
        let (timeout, rollback_task) =
            resettable_delay_for(DESCRIPTION_APPROVE_TIMEOUT);
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
        rollback_task
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
    /// On every SDP offer update this field should be reseted to `false` and
    /// if this field doesn't transits into `true` within [`APPROVE_TIMEOUT`],
    /// then [`Inner::current_sdp`] should be rollbacked to the
    /// [`Inner::prev_sdp`].
    approved: Rc<ObservableCell<bool>>,

    /// Timeout of the [`Inner::approved`] transition.
    rollback_task_handle: RefCell<Option<ResettableDelayHandle>>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            prev_sdp: RefCell::new(None),
            current_sdp: ObservableCell::new(None),
            approved: Rc::new(ObservableCell::new(true)),
            rollback_task_handle: RefCell::new(None),
        }
    }
}
