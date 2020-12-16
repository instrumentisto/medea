use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{channel::mpsc, future::Either, stream::LocalBoxStream};
use medea_reactive::ObservableCell;
use wasm_bindgen_futures::spawn_local;

use crate::utils::{resettable_delay_for, ResettableDelayHandle};

const APPROVE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub enum Sdp {
    Rollback,
    Offer(String),
}

struct Inner {
    current_offer: Option<String>,
    old_offer: Option<String>,
    approved: ObservableCell<bool>,
    new_local_sdp: Vec<mpsc::UnboundedSender<Sdp>>,
    timeout_handle: Option<ResettableDelayHandle>,
}

impl Inner {
    fn new() -> Self {
        Self {
            current_offer: None,
            old_offer: None,
            approved: ObservableCell::new(true),
            new_local_sdp: Vec::new(),
            timeout_handle: None,
        }
    }

    fn send_sdp(&mut self, sdp: Sdp) {
        self.new_local_sdp
            .retain(|s| s.unbounded_send(sdp.clone()).is_ok());
    }

    fn on_new_local_sdp(&mut self) -> LocalBoxStream<'static, Sdp> {
        let (tx, rx) = mpsc::unbounded();
        self.new_local_sdp.push(tx);

        Box::pin(rx)
    }

    fn rollback(&mut self) {
        self.current_offer = self.old_offer.take();
        self.approved.set(true);

        self.send_sdp(Sdp::Rollback);
    }

    fn approve(&mut self) {
        self.approved.set(true);
    }
}

#[derive(Clone)]
pub struct LocalSdp(Rc<RefCell<Inner>>);

impl LocalSdp {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Inner::new())))
    }

    pub fn on_new_local_sdp(&self) -> LocalBoxStream<'static, Sdp> {
        self.0.borrow_mut().on_new_local_sdp()
    }

    pub fn rollback(&self) {
        self.0.borrow_mut().rollback()
    }

    pub fn update_offer(&self, offer: String) {
        let (timeout, timeout_handle) = resettable_delay_for(APPROVE_TIMEOUT);
        self.0.borrow_mut().approved.set(false);
        self.0.borrow_mut().timeout_handle.replace(timeout_handle);
        spawn_local({
            let this = self.clone();
            let approved = self.0.borrow().approved.when_eq(true);
            async move {
                match futures::future::select(approved, Box::pin(timeout)).await
                {
                    Either::Left(_) => (),
                    Either::Right(_) => {
                        this.rollback();
                    }
                }
            }
        });
        let old_offer =
            self.0.borrow_mut().current_offer.replace(offer.clone());
        self.0.borrow_mut().old_offer = old_offer;

        self.0.borrow_mut().send_sdp(Sdp::Offer(offer));
    }

    pub fn approve(&self) {
        self.0.borrow_mut().approve()
    }
}
