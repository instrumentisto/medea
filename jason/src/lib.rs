use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use web_sys::{console};
use wasm_bindgen::prelude::*;

mod transport;
mod utils;

use transport::{protocol::Event as MedeaEvent, Transport};
use futures::stream::Stream;

// When the `console_error_panic_hook` feature is enabled, we can call the
// `set_panic_hook` function at least once during initialization, and then
// we will get better error messages if our code ever panics.
//
// For more details see
// https://github.com/rustwasm/console_error_panic_hook#readme
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;
use wasm_bindgen_futures::spawn_local;
use crate::transport::protocol::DirectionalTrack;
use std::rc::Rc;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct Jason {
    //TODO: multiple session will require some kind of TransportManager
    transport: Option<Transport>,
}

#[wasm_bindgen]
pub struct SessionHandler {
    transport: Rc<Option<Transport>>,
    tx: UnboundedSender<MedeaEvent>,
    rx: UnboundedReceiver<MedeaEvent>,
}

impl SessionHandler {
    fn new() -> SessionHandler {
        let (tx, rx) = unbounded();

        SessionHandler { transport: Rc::new(None), tx, rx }
    }
}

struct InnerSession {

}

#[wasm_bindgen]
impl Jason {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        set_panic_hook();
        Self { transport: None }
    }

    pub fn init_session(&mut self, token: String) {
        let mut transport = Transport::new(token, 3000);
        transport.init();

        let handler = SessionHandler::new();

        transport.add_sub(handler.tx.clone());

        let poll = handler.rx.for_each(|event| {
            console::log(&js_sys::Array::from(&JsValue::from_str(&format!("{:?}", event))));
            match event {
                MedeaEvent::PeerCreated {        peer_id,sdp_offer,tracks} => {
//                    &self.on_peer_created(peer_id, sdp_offer, tracks);
                },
                MedeaEvent::SdpAnswerMade {peer_id, sdp_answer} => {

                },
                MedeaEvent::IceCandidateDiscovered {peer_id, candidate} => {

                },
                MedeaEvent::PeersRemoved {peer_ids} => {

                },
            };

            Ok(())
        });

        spawn_local(poll);

        self.transport = Some(transport);
    }

    fn on_peer_created(&self, peer_id: u64, sdp_offer: Option<String>, tracks: Vec<DirectionalTrack>) {}
//    fn on_sdp_answer() {}
//    fn on_ice_candidate_discovered() {}
//    fn peers_removed() {}
}
