//! Represents Medea room.

use futures::{
    future::{Future, IntoFuture},
    stream::Stream,
};
use protocol::{Event, IceCandidate, Track};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::console;

use std::rc::{Rc, Weak};

use crate::rpc::RPCClient;
use std::cell::RefCell;

#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
/// Room handle accessible from JS.
pub struct RoomHandle(Weak<RefCell<InnerRoom>>);

#[wasm_bindgen]
impl RoomHandle {}

/// Room handle being used by Rust external modules.
pub struct Room(Rc<RefCell<InnerRoom>>);

impl Room {
    /// Creates new [`Room`] associating it with provided [`RpcClient`].
    pub fn new(rpc: &Rc<RPCClient>) -> Self {
        let room = Rc::new(RefCell::new(InnerRoom::new(Rc::clone(&rpc))));

        let inner = Rc::clone(&room);

        let process_msg_task = rpc
            .subscribe()
            .for_each(move |event| {
                // TODO: macro for convenient dispatch
                let mut inner = inner.borrow_mut();
                match event {
                    Event::PeerCreated {
                        peer_id,
                        sdp_offer,
                        tracks,
                    } => {
                        inner.on_peer_created(peer_id, &sdp_offer, &tracks);
                    }
                    Event::SdpAnswerMade {
                        peer_id,
                        sdp_answer,
                    } => {
                        inner.on_sdp_answer(peer_id, &sdp_answer);
                    }
                    Event::IceCandidateDiscovered { peer_id, candidate } => {
                        inner.on_ice_candidate_discovered(peer_id, &candidate);
                    }
                    Event::PeersRemoved { peer_ids } => {
                        inner.on_peers_removed(&peer_ids);
                    }
                };
                Ok(())
            })
            .into_future()
            .then(|_| Ok(()));

        // Spawns Promise in JS, does not provide any handles, so current way to
        // stop this stream is to drop all connected Senders.
        spawn_local(process_msg_task);

        Self(room)
    }

    /// Creates new [`RoomHandle`] used by JS side. You can create them as many
    /// as you need.
    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::downgrade(&self.0))
    }
}

// Actual room. Shared between JS-side handle (['RoomHandle']) and Rust-side
// handle (['Room']). Manages concrete RTCPeerConnections, handles Medea events.
struct InnerRoom {
    rpc: Rc<RPCClient>,
}

impl InnerRoom {
    fn new(rpc: Rc<RPCClient>) -> Self {
        Self { rpc }
    }

    /// Creates RTCPeerConnection with provided ID.
    fn on_peer_created(
        &mut self,
        _peer_id: u64,
        _sdp_offer: &Option<String>,
        _tracks: &[Track],
    ) {
        console::log_1(&JsValue::from_str("on_peer_created invoked"));
    }

    /// Applies specified SDP Answer to specified RTCPeerConnection.
    fn on_sdp_answer(&mut self, _peer_id: u64, _sdp_answer: &str) {
        console::log_1(&JsValue::from_str("on_sdp_answer invoked"));
    }

    /// Applies specified ICE Candidate to specified RTCPeerConnection.
    fn on_ice_candidate_discovered(
        &mut self,
        _peer_id: u64,
        _candidate: &IceCandidate,
    ) {
        console::log_1(&JsValue::from_str(
            "on_ice_candidate_discovered invoked",
        ));
    }

    /// Disposes specified RTCPeerConnection's.
    fn on_peers_removed(&mut self, _peer_ids: &[u64]) {
        console::log_1(&JsValue::from_str("on_peers_removed invoked"));
    }
}

impl Drop for InnerRoom {
    fn drop(&mut self) {
        // Drops event handling task.
        self.rpc.unsub();
    }
}
