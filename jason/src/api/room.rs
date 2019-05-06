//! Represents Medea room.

use futures::{
    future::{Future, IntoFuture},
    stream::Stream,
};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::console;

use std::{cell::RefCell, rc::Rc};

use crate::rpc::{protocol::DirectionalTrack, protocol::Event, RPCClient};
use crate::utils::WasmErr;

#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
/// Room handle accessible from JS.
pub struct RoomHandle(Rc<RefCell<Option<InnerRoom>>>);

#[wasm_bindgen]
impl RoomHandle {
    /// on_local_media = function(error, stream)
    pub fn on_local_stream(&mut self, on_local_media: js_sys::Function) {
        match self.0.borrow_mut().as_mut() {
            Some(inner) => {
                inner.on_local_media.replace(on_local_media);
            }
            None => {
                on_local_media.call2(
                    &JsValue::NULL,
                    &JsValue::NULL,
                    &WasmErr::from_str("Detached state").into(),
                );
            }
        }
    }
}

/// Room handle being used by Rust external modules.
pub struct Room(Rc<RefCell<Option<InnerRoom>>>);

impl Room {
    pub fn new(rpc: Rc<RPCClient>) -> Self {
        Self(InnerRoom::new(rpc))
    }

    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::clone(&self.0))
    }

    /// Subscribes to provided RpcTransport messages.
    pub fn subscribe(&self, rpc: &RPCClient) {
        let inner = Rc::clone(&self.0);

        let process_msg_task = rpc
            .subscribe()
            .for_each(move |event| {
                // TODO: macro for convenient dispatch
                match inner.borrow_mut().as_mut() {
                    Some(inner) => {
                        match event {
                            Event::PeerCreated {
                                peer_id,
                                sdp_offer,
                                tracks,
                            } => {
                                inner.on_peer_created(
                                    peer_id, &sdp_offer, &tracks,
                                );
                            }
                            Event::SdpAnswerMade {
                                peer_id,
                                sdp_answer,
                            } => {
                                inner.on_sdp_answer(peer_id, &sdp_answer);
                            }
                            Event::IceCandidateDiscovered {
                                peer_id,
                                candidate,
                            } => {
                                inner.on_ice_candidate_discovered(
                                    peer_id, &candidate,
                                );
                            }
                            Event::PeersRemoved { peer_ids } => {
                                inner.on_peers_removed(&peer_ids);
                            }
                        };
                        Ok(())
                    }
                    None => {
                        // InnerSession is gone, which means that Room was
                        // dropped. Not supposed to happen, since InnerSession
                        // should drop its tx by unsubbing from RpcClient.
                        Err(())
                    }
                }
            })
            .into_future()
            .then(|_| Ok(()));

        // Spawns Promise in JS, does not provide any handles, so current way to
        // stop this stream is to drop all connected Senders.
        spawn_local(process_msg_task);
    }
}

// Actual room. Shared between JS-side handle (['RoomHandle']) and Rust-side
// handle (['Room']). Manages concrete RTCPeerConnections, handles Medea events.
struct InnerRoom {
    rpc: Rc<RPCClient>,
    on_local_media: Option<js_sys::Function>,
}

impl InnerRoom {
    fn new(rpc: Rc<RPCClient>) -> Rc<RefCell<Option<Self>>> {
        Rc::new(RefCell::new(Some(Self {
            rpc,
            on_local_media: None,
        })))
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

impl Drop for Room {
    fn drop(&mut self) {
        // Drop InnerRoom, invalidates all spawned RoomHandler's.
        self.0.borrow_mut().take();
    }
}

impl Drop for InnerRoom {
    fn drop(&mut self) {
        // Drops event handling task.
        self.rpc.unsub();
    }
}
