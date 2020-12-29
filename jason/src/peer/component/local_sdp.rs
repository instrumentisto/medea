//! Implementation of the local SDP offer state.

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{future, future::Either, stream::LocalBoxStream, StreamExt};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::utils::{resettable_delay_for, ResettableDelayHandle};

const APPROVE_TIMEOUT: Duration = Duration::from_secs(10);

/// Inner for the [`LocalSdp`].
#[derive(Debug)]
struct Inner {
    /// Current SDP offer applied on the [`PeerConnection`], but it can be not
    /// approved by server (see [`Inner::approved`]).
    current_offer: ObservableCell<Option<String>>,

    /// Previous SDP offer to which this [`Inner::current_offer`] can be
    /// transited if server doesn't approved [`Inner::current_offer`].
    prev_offer: Option<String>,

    /// Flag which indicates that Media Server approved this SDP
    /// [`Inner::current_offer`].
    ///
    /// On every SDP offer update this field should be reseted to `false` and
    /// if this field doesn't transits into `true` within [`APPROVE_TIMEOUT`],
    /// then [`Inner::current_offer`] should be rollbacked to the
    /// [`Inner::prev_offer`].
    approved: ObservableCell<bool>,

    /// Flag which indicates that new SDP offer needed after rollback is
    /// completed.
    restart_needed: bool,

    /// Timeout of the [`Inner::approved`] transition.
    timeout_handle: Option<ResettableDelayHandle>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            prev_offer: None,
            current_offer: ObservableCell::new(None),
            approved: ObservableCell::new(true),
            timeout_handle: None,
            restart_needed: false,
        }
    }
}

impl Inner {
    /// Rollbacks [`LocalSdp`] to the previous one.
    fn rollback(&mut self) {
        self.approved.set(true);
        self.current_offer.set(self.prev_offer.clone());
    }

    /// Sets [`Inner::approved`] flag to the `true`.
    fn approve(&mut self) {
        self.approved.set(true);
    }
}

/// Wrapper around SDP offer which stores previous SDP approved SDP offer and
/// can rollback to it on timeout.
///
/// If you update [`LocalSdp`] then it will wait for server approve
/// ([`LocalSdp::approve`]). If Media Server approve wasn't received within
/// timeout, then SDP offer will be rollbacked to the previous one.
#[derive(Clone, Default, Debug)]
pub struct LocalSdp(Rc<RefCell<Inner>>);

impl LocalSdp {
    /// Returns new [`LocalSdp`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops [`LocalSdp`] rollback timeout.
    pub fn stop_timeout(&self) {
        if let Some(handle) = self.0.borrow().timeout_handle.as_ref() {
            handle.stop();
        }
    }

    /// Resumes [`LocalSdp`] rollback timeout.
    pub fn resume_timeout(&self) {
        if let Some(handle) = self.0.borrow().timeout_handle.as_ref() {
            handle.reset();
        }
    }

    /// Returns [`Stream`] into which `()` will be sent on every SDP offer
    /// approve.
    pub fn on_approve(&self) -> LocalBoxStream<'static, ()> {
        Box::pin(self.0.borrow().approved.subscribe().filter_map(|approved| {
            future::ready(if approved { Some(()) } else { None })
        }))
    }

    /// Rollbacks [`LocalSdp`] to the previous one.
    pub fn rollback(&self) {
        self.0.borrow_mut().rollback()
    }

    /// Handles SDP offer update received from the server.
    pub fn update_offer_by_server(&self, new_offer: &Option<String>) {
        let approved = new_offer
            .as_ref()
            .map(|new_offer| {
                self.0.borrow().current_offer.mutate(|current_offer| {
                    current_offer
                        .as_ref()
                        .map(|c| new_offer == c)
                        .unwrap_or_default()
                })
            })
            .unwrap_or_default();
        let not_approved =
            new_offer.is_none() && !self.0.borrow().approved.get();
        if not_approved {
            self.0.borrow_mut().restart_needed = true;
            self.rollback();
        } else if approved {
            self.0.borrow().approved.set(true);
        }
    }

    /// Updates current SDP offer to the provided one.
    pub fn update_offer_by_client(&self, new_offer: String) {
        let (timeout, timeout_handle) = resettable_delay_for(APPROVE_TIMEOUT);
        self.0.borrow_mut().approved.set(false);
        self.0.borrow_mut().timeout_handle.replace(timeout_handle);
        self.0.borrow_mut().restart_needed = false;
        spawn_local({
            let this = self.clone();
            let approved = self.0.borrow().approved.when_eq(true);
            async move {
                match future::select(approved, Box::pin(timeout)).await {
                    Either::Left(_) => (),
                    Either::Right(_) => {
                        this.rollback();
                    }
                }
            }
        });
        let prev_offer = self
            .0
            .borrow_mut()
            .current_offer
            .mutate(|mut o| o.replace(new_offer));
        self.0.borrow_mut().prev_offer = prev_offer;
    }

    /// Approves current [`LocalSdp`] offer.
    pub fn approve(&self, sdp_offer: &str) {
        let mut inner = self.0.borrow_mut();
        let is_approved = inner.current_offer.mutate(|current| {
            current.as_ref().map(String::as_str) == Some(sdp_offer)
        });
        if is_approved {
            inner.approve()
        }
    }

    /// Returns current SDP offer.
    pub fn current(&self) -> Option<String> {
        self.0.borrow().current_offer.get()
    }

    /// Returns [`LocalBoxStream`] into which all current SDP offer updates will
    /// be sent.
    pub fn subscribe(&self) -> LocalBoxStream<'static, Option<String>> {
        self.0.borrow().current_offer.subscribe()
    }

    /// Returns `true` if [`LocalSdp`] current SDP offer equal to the previous
    /// SDP offer and they both is `Some`.
    pub fn is_rollback(&self) -> bool {
        let inner = self.0.borrow();
        inner.current_offer.mutate(|c| {
            c.as_ref().map_or(false, |current| {
                inner
                    .prev_offer
                    .as_ref()
                    .map(|prev| prev == current)
                    .unwrap_or_default()
            })
        })
    }

    #[inline]
    pub fn is_restart_needed(&self) -> bool {
        self.0.borrow().restart_needed
    }
}
