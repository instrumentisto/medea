//! Represents Medea room.

use futures::{stream::Stream, sync::mpsc::unbounded};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::console;

use std::{cell::RefCell, rc::Rc};

use crate::transport::{
    protocol::DirectionalTrack, protocol::Event as MedeaEvent, Transport,
};

#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
/// Room handle accessible from JS.
pub struct RoomHandle(Rc<RefCell<InnerRoom>>);

#[wasm_bindgen]
impl RoomHandle {}

pub struct Room(Rc<RefCell<InnerRoom>>);

impl Room {
    pub fn new(transport: Rc<Transport>) -> Self {
        Self(InnerRoom::new(transport))
    }

    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::clone(&self.0))
    }

    /// Subscribes to provided transport messages.
    pub fn subscribe(&self, transport: &Transport) {
        let (tx, rx) = unbounded();
        transport.add_sub(tx);

        let inner = Rc::clone(&self.0);

        let process_msg_task = rx.for_each(move |event| {
            match event {
                MedeaEvent::PeerCreated {
                    peer_id,
                    sdp_offer,
                    tracks,
                } => {
                    inner
                        .borrow_mut()
                        .on_peer_created(peer_id, &sdp_offer, &tracks);
                }
                MedeaEvent::SdpAnswerMade {
                    peer_id,
                    sdp_answer,
                } => {
                    inner.borrow_mut().on_sdp_answer(peer_id, &sdp_answer);
                }
                MedeaEvent::IceCandidateDiscovered { peer_id, candidate } => {
                    inner
                        .borrow_mut()
                        .on_ice_candidate_discovered(peer_id, &candidate);
                }
                MedeaEvent::PeersRemoved { peer_ids } => {
                    inner.borrow_mut().on_peers_removed(&peer_ids);
                }
            };

            Ok(())
        });

        spawn_local(process_msg_task);
    }
}

struct InnerRoom {
    _transport: Rc<Transport>,
}

impl InnerRoom {
    fn new(transport: Rc<Transport>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            _transport: transport,
        }))
    }

    /// Creates RTCPeerConnection with provided ID.
    fn on_peer_created(
        &mut self,
        _peer_id: u64,
        _sdp_offer: &Option<String>,
        _tracks: &[DirectionalTrack],
    ) {
        console::log_1(&JsValue::from_str("on_peer_created invoked"));
    }

    /// Applies specified SDP Answer to specified RTCPeerConnection.
    fn on_sdp_answer(&mut self, _peer_id: u64, _sdp_answer: &str) {
        console::log_1(&JsValue::from_str("on_sdp_answer invoked"));
    }

    /// Applies specified ICE Candidate to specified RTCPeerConnection.
    fn on_ice_candidate_discovered(&mut self, _peer_id: u64, _candidate: &str) {
        console::log_1(&JsValue::from_str(
            "on_ice_candidate_discovered invoked",
        ));
    }

    /// Disposes specified RTCPeerConnection's.
    fn on_peers_removed(&mut self, _peer_ids: &[u64]) {
        console::log_1(&JsValue::from_str("on_peers_removed invoked"));
    }
}
