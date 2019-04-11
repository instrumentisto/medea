use futures::{
    stream::Stream,
    sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::console;

use std::{cell::RefCell, rc::Rc};

use crate::transport::{
    protocol::DirectionalTrack, protocol::Event as MedeaEvent, Transport,
};

#[wasm_bindgen]
pub struct SessionHandle {
    inner: Session,
}

#[wasm_bindgen]
impl SessionHandle {}

pub struct Session(Rc<RefCell<InnerSession>>);

impl Session {
    pub fn new(transport: Rc<Transport>) -> Self {
        Self {
            0: InnerSession::new(transport),
        }
    }

    pub fn new_handle(&self) -> SessionHandle {
        SessionHandle {
            inner: Session {
                0: Rc::clone(&self.0),
            },
        }
    }

    pub fn subscribe(&self, transport: &Transport) {
        let mut inner = self.0.borrow_mut();

        transport.add_sub(inner.tx.clone());

        let rx = inner.rx.take().unwrap();

        let inner = Rc::clone(&self.0);
        let poll = rx.for_each(move |event| {
            match event {
                MedeaEvent::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks,
                } => {
                    inner
                        .borrow_mut()
                        .on_peer_created(peer_id, sdp_offer, tracks);
                }
                MedeaEvent::SdpAnswerMade {
                    peer_id,
                    sdp_answer,
                } => {
                    inner.borrow_mut().on_sdp_answer(peer_id, sdp_answer);
                }
                MedeaEvent::IceCandidateDiscovered { peer_id, candidate } => {
                    inner
                        .borrow_mut()
                        .on_ice_candidate_discovered(peer_id, candidate);
                }
                MedeaEvent::PeersRemoved { peer_ids } => {
                    inner.borrow_mut().on_peers_removed(peer_ids);
                }
            };

            Ok(())
        });

        spawn_local(poll);
    }
}

struct InnerSession {
    transport: Rc<Transport>,
    tx: UnboundedSender<MedeaEvent>,
    rx: Option<UnboundedReceiver<MedeaEvent>>,
}

impl InnerSession {
    fn new(transport: Rc<Transport>) -> Rc<RefCell<Self>> {
        let (tx, rx) = unbounded();

        Rc::new(RefCell::new(Self {
            transport,
            tx,
            rx: Some(rx),
        }))
    }

    fn on_peer_created(
        &mut self,
        peer_id: u64,
        sdp_offer: Option<String>,
        tracks: Vec<DirectionalTrack>,
    ) {
        console::log(&js_sys::Array::from(&JsValue::from_str(
            "on_peer_created invoked",
        )));
    }

    fn on_sdp_answer(&mut self, peer_id: u64, sdp_answer: String) {
        console::log(&js_sys::Array::from(&JsValue::from_str(
            "on_sdp_answer invoked",
        )));
    }

    fn on_ice_candidate_discovered(&mut self, peer_id: u64, candidate: String) {
        console::log(&js_sys::Array::from(&JsValue::from_str(
            "on_ice_candidate_discovered invoked",
        )));
    }

    fn on_peers_removed(&mut self, peer_ids: Vec<u64>) {
        console::log(&js_sys::Array::from(&JsValue::from_str(
            "on_peers_removed invoked",
        )));
    }
}
