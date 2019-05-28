//! Medea room.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use futures::{
    future::{Future, IntoFuture},
    stream::Stream,
};
use medea_client_api_proto::{EventHandler, IceCandidate, Track};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::console;

use crate::rpc::RpcClient;

#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
/// Room handle accessible from JS.
pub struct RoomHandle(Weak<RefCell<InnerRoom>>);

#[wasm_bindgen]
impl RoomHandle {}

impl EventHandler for RoomHandle {
    /// Creates RTCPeerConnection with provided ID.
    fn on_peer_created(
        &mut self,
        _peer_id: u64,
        _sdp_offer: Option<String>,
        _tracks: Vec<Track>,
    ) {
        console::log_1(&JsValue::from_str("on_peer_created invoked"));
    }

    /// Applies specified SDP Answer to specified RTCPeerConnection.
    fn on_sdp_answer_made(&mut self, _peer_id: u64, _sdp_answer: String) {
        console::log_1(&JsValue::from_str("on_sdp_answer invoked"));
    }

    /// Applies specified ICE Candidate to specified RTCPeerConnection.
    fn on_ice_candidate_discovered(
        &mut self,
        _peer_id: u64,
        _candidate: IceCandidate,
    ) {
        console::log_1(&JsValue::from_str(
            "on_ice_candidate_discovered invoked",
        ));
    }

    /// Disposes specified RTCPeerConnection's.
    fn on_peers_removed(&mut self, _peer_ids: Vec<u64>) {
        console::log_1(&JsValue::from_str("on_peers_removed invoked"));
    }
}

/// Room handle being used by Rust external modules.
pub struct Room(Rc<RefCell<InnerRoom>>);

impl Room {
    /// Creates new [`Room`] associating it with provided [`RpcClient`].
    pub fn new(rpc: &Rc<RpcClient>) -> Self {
        let room = Rc::new(RefCell::new(InnerRoom::new(Rc::clone(&rpc))));
        let mut inner = RoomHandle(Rc::downgrade(&room));

        let process_msg_task = rpc
            .subscribe()
            .for_each(move |event| {
                event.dispatch(&mut inner);
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

/// Actual room. Manages concrete `RTCPeerConnection`s, handles Medea events.
///
/// Shared between JS-side handle ([`RoomHandle`])
/// and Rust-side handle ([`Room`]).
struct InnerRoom {
    rpc: Rc<RpcClient>,
}

impl InnerRoom {
    fn new(rpc: Rc<RpcClient>) -> Self {
        Self { rpc }
    }
}

impl Drop for InnerRoom {
    /// Drops event handling task.
    fn drop(&mut self) {
        self.rpc.unsub();
    }
}
