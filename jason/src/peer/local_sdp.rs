//! Implementation of the local SDP offer state.

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{
    channel::mpsc,
    future,
    future::{Either, LocalBoxFuture},
    stream::LocalBoxStream,
    FutureExt, StreamExt,
};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::utils::{resettable_delay_for, ResettableDelayHandle};

const APPROVE_TIMEOUT: Duration = Duration::from_secs(10);

/// SDP offer which should be applied.
#[derive(Clone, Debug)]
pub enum Sdp {
    /// SDP offer should be rollbacked.
    Rollback(bool),

    /// New SDP offer should be set.
    Offer(String),
}

/// Inner for the [`LocalSdp`].
struct Inner {
    /// Current SDP offer applied on the [`PeerConnection`], but it can be not
    /// approved by server (see [`Inner::approved`]).
    current_offer: Option<String>,

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

    /// [`mpsc::UnboundedSender`]s for the [`Sdp`] updates.
    local_sdp_update_txs: Vec<mpsc::UnboundedSender<Sdp>>,

    /// Timeout of the [`Inner::approved`] transition.
    timeout_handle: Option<ResettableDelayHandle>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            current_offer: None,
            prev_offer: None,
            approved: ObservableCell::new(true),
            local_sdp_update_txs: Vec::new(),
            timeout_handle: None,
        }
    }
}

impl Inner {
    /// Sends SDP offer update to the [`Inner::local_sdp_update_txs`].
    fn send_sdp(&mut self, sdp: &Sdp) {
        self.local_sdp_update_txs
            .retain(|s| s.unbounded_send(sdp.clone()).is_ok());
    }

    /// Returns [`LocalBoxStream`] into which all [`Sdp`] update will be sent.
    fn on_new_local_sdp(&mut self) -> LocalBoxStream<'static, Sdp> {
        let (tx, rx) = mpsc::unbounded();
        self.local_sdp_update_txs.push(tx);

        Box::pin(rx)
    }

    /// Rollbacks [`LocalSdp`] to the previous one.
    fn rollback(&mut self, is_restart: bool) {
        self.current_offer = self.prev_offer.take();
        self.approved.set(true);

        self.send_sdp(&Sdp::Rollback(is_restart));
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
#[derive(Clone, Default)]
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

    /// Returns [`LocalBoxStream`] into which all [`Sdp`] update will be sent.
    pub fn on_new_local_sdp(&self) -> LocalBoxStream<'static, Sdp> {
        self.0.borrow_mut().on_new_local_sdp()
    }

    pub fn on_approve(&self) -> LocalBoxStream<'static, ()> {
        Box::pin(self.0.borrow().approved.subscribe().filter_map(|approved| {
            future::ready(if approved { Some(()) } else { None })
        }))
    }

    /// Rollbacks [`LocalSdp`] to the previous one.
    pub fn rollback(&self, is_restart: bool) {
        self.0.borrow_mut().rollback(is_restart)
    }

    pub fn update_offer_by_server(&self, new_offer: Option<String>) {
        if self.0.borrow().prev_offer == new_offer {
            self.rollback(true);
        }
        if self.0.borrow().current_offer == new_offer {
            self.approve();
        }

        // TODO (evdokimovs): everything else is unreachable. But what we will
        // do with it?
    }

    /// Updates current SDP offer to the provided one.
    pub fn update_offer_by_client(&self, new_offer: String) {
        let (timeout, timeout_handle) = resettable_delay_for(APPROVE_TIMEOUT);
        self.0.borrow_mut().approved.set(false);
        self.0.borrow_mut().timeout_handle.replace(timeout_handle);
        spawn_local({
            let this = self.clone();
            let approved = self.0.borrow().approved.when_eq(true);
            async move {
                match future::select(approved, Box::pin(timeout)).await {
                    Either::Left(_) => (),
                    Either::Right(_) => {
                        this.rollback(false);
                    }
                }
            }
        });
        let prev_offer =
            self.0.borrow_mut().current_offer.replace(new_offer.clone());
        self.0.borrow_mut().prev_offer = prev_offer;

        self.0.borrow_mut().send_sdp(&Sdp::Offer(new_offer));
    }

    /// Approves current [`LocalSdp`] offer.
    pub fn approve(&self) {
        self.0.borrow_mut().approve()
    }

    /// Returns current SDP offer.
    pub fn current(&self) -> Option<String> {
        self.0.borrow().current_offer.clone()
    }
}
