//! Represents Medea room.

use futures::{
    future::{Future, IntoFuture},
    stream::Stream,
};
use protocol::{Directional, Event, IceCandidate};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::console;

use std::{cell::RefCell, rc::Rc};

use crate::{
    media::{
        MediaCaps, MediaManager, MediaStreamHandle, PeerId, PeerRepository,
    },
    rpc::RPCClient,
    utils::{Callback, WasmErr},
};

#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
/// Room handle accessible from JS.
pub struct RoomHandle(Rc<RefCell<Option<InnerRoom>>>);

#[wasm_bindgen]
impl RoomHandle {
    pub fn on_local_stream(&mut self, f: js_sys::Function) {
        if let Some(inner) = self.0.borrow_mut().as_mut() {
            inner.on_local_media.set_func(f);
        } else {
            let f: Callback<i32, WasmErr> = f.into();
            f.call_err(WasmErr::from_str("Detached state"));
        }
    }
}

/// Room handle being used by Rust external modules.
pub struct Room(Rc<RefCell<Option<InnerRoom>>>);

impl Room {
    pub fn new(rpc: Rc<RPCClient>, media_manager: Rc<MediaManager>) -> Self {
        Self(InnerRoom::new(rpc, media_manager))
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
                        // should drop its tx by unsubbing from RpcClient,
                        // meaning that current stream is supposed to resolve
                        // before InnerSession drop.
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

/// Actual room. Shared between JS-side handle ([`RoomHandle`]) and Rust-side
/// handle (['Room']). Manages concrete `RTCPeerConnections`, handles Medea
/// events.
struct InnerRoom {
    rpc: Rc<RPCClient>,
    media_manager: Rc<MediaManager>,
    peers: PeerRepository,
    on_local_media: Rc<Callback<MediaStreamHandle, WasmErr>>,
}

impl InnerRoom {
    fn new(
        rpc: Rc<RPCClient>,
        media_manager: Rc<MediaManager>,
    ) -> Rc<RefCell<Option<Self>>> {
        Rc::new(RefCell::new(Some(Self {
            rpc,
            media_manager,
            peers: PeerRepository::default(),
            on_local_media: Rc::new(Callback::new()),
        })))
    }

    fn on_local_media(&self, f: js_sys::Function) {
        self.on_local_media.set_func(f);
    }

    /// Creates RTCPeerConnection with provided ID.
    fn on_peer_created(
        &mut self,
        _peer_id: u64,
        _sdp_offer: &Option<String>,
        _tracks: &[Directional],
    ) {
        let on_local_media = Rc::clone(&self.on_local_media);
        match MediaCaps::new(true, true) {
            Err(err) => {
                on_local_media.call_err(err);
            }
            Ok(caps) => {
                let fut =
                    self.media_manager.get_stream(&caps).then(move |result| {
                        on_local_media
                            .call(result.map(|stream| stream.new_handle()));
                        Ok(())
                    });
                spawn_local(fut);
            }
        }

        console::log_1(&JsValue::from_str("on_peer_created invoked"));
    }

    /// Applies specified SDP Answer to specified RTCPeerConnection.
    fn on_sdp_answer(&mut self, _peer_id: u64, _sdp_answer: &str) {
        console::log_1(&JsValue::from_str("on_sdp_answer invoked"));
    }

    /// Applies specified ICE Candidate to specified RTCPeerConnection.
    fn on_ice_candidate_discovered(
        &mut self,
        peer_id: PeerId,
        candidate: &IceCandidate,
    ) {
        if let Some(peer) = self.peers.get_peer(peer_id) {
            peer.add_ice_candidate(candidate);
        } else {
            // TODO: no peer, whats next?
            WasmErr::from_str(format!("Peer with id {} doesnt exist", peer_id));
        }
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
