use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use web_sys::console;
use wasm_bindgen::prelude::*;

mod transport;
mod utils;

use transport::{protocol::Event as MedeaEvent, Transport, protocol::DirectionalTrack};
use futures::stream::Stream;

use wasm_bindgen_futures::spawn_local;
use std::rc::Rc;
use std::cell::RefCell;

// When the `console_error_panic_hook` feature is enabled, we can call the
// `set_panic_hook` function at least once during initialization, and then
// we will get better error messages if our code ever panics.
//
// For more details see
// https://github.com/rustwasm/console_error_panic_hook#readme
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;


// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct Jason {
    transport: Option<Rc<Transport>>,
    sessions: Vec<Rc<Session>>,
}

#[wasm_bindgen]
impl Jason {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        Self {
            transport: None,
            sessions: vec![],
        }
    }

    pub fn init_session(&mut self, token: String) -> SessionHandle {
        let mut transport = Transport::new(token, 3000);
        transport.init();
        let transport = Rc::new(transport);

        let session = Session::new(Rc::clone(&transport));
        session.subscribe(&transport);

        let handle = session.new_handle();

        self.sessions.push(Rc::new(session));
        self.transport = Some(transport);

        handle
    }
}

#[wasm_bindgen]
pub struct SessionHandle {
    inner: Session
}

#[wasm_bindgen]
impl SessionHandle {}

struct Session(Rc<RefCell<InnerSession>>);

impl Session {
    fn new(transport: Rc<Transport>) -> Self {
        Self {
            0: InnerSession::new(transport)
        }
    }

    fn new_handle(&self) -> SessionHandle {
        SessionHandle {
            inner: Session { 0: Rc::clone(&self.0) }
        }
    }

    fn subscribe(&self, transport: &Transport) {

        let mut inner = self.0.borrow_mut();

        transport.add_sub(inner.tx.clone());

        let rx = inner.rx.take().unwrap();

        let inner = Rc::clone(&self.0);
        let poll = rx.for_each(move |event| {
            match event {
                MedeaEvent::PeerCreated { peer_id, sdp_offer, tracks } => {
                    inner.borrow_mut().on_peer_created(peer_id, sdp_offer, tracks);
                }
                MedeaEvent::SdpAnswerMade { peer_id, sdp_answer } => {
                    inner.borrow_mut().on_sdp_answer(peer_id, sdp_answer);
                }
                MedeaEvent::IceCandidateDiscovered { peer_id, candidate } => {
                    inner.borrow_mut().on_ice_candidate_discovered(peer_id, candidate);
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

    fn on_peer_created(&mut self, peer_id: u64, sdp_offer: Option<String>, tracks: Vec<DirectionalTrack>) {
        console::log(&js_sys::Array::from(&JsValue::from_str("on_peer_created invoked")));
    }
    fn on_sdp_answer(&mut self, peer_id: u64, sdp_answer: String) {
        console::log(&js_sys::Array::from(&JsValue::from_str("on_sdp_answer invoked")));
    }
    fn on_ice_candidate_discovered(&mut self, peer_id: u64, candidate: String) {
        console::log(&js_sys::Array::from(&JsValue::from_str("on_ice_candidate_discovered invoked")));
    }
    fn on_peers_removed(&mut self, peer_ids: Vec<u64>) {
        console::log(&js_sys::Array::from(&JsValue::from_str("on_peers_removed invoked")));
    }
}
