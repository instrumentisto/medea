//! Represents Medea room.

use futures::{
    future::{Future, IntoFuture},
    stream::Stream,
};
use protocol::Command;
use protocol::{Directional, Event, IceCandidate};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::console;

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::{
    media::{
        MediaCaps, MediaManager, MediaStream, MediaStreamHandle, PeerId,
        PeerRepository,
    },
    rpc::RPCClient,
    utils::{Callback, WasmErr},
};

#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
/// Room handle accessible from JS.
pub struct RoomHandle(Weak<RefCell<InnerRoom>>);

#[wasm_bindgen]
impl RoomHandle {
    pub fn on_local_stream(&mut self, f: js_sys::Function) {
        if let Some(inner) = self.0.upgrade() {
            inner.borrow_mut().on_local_media.set_func(f);
        } else {
            let f: Callback<i32, WasmErr> = f.into();
            f.call_err(WasmErr::from_str("Detached state"));
        }
    }
}

/// Room handle being used by Rust external modules.
pub struct Room(Rc<RefCell<InnerRoom>>);

impl Room {
    pub fn new(rpc: Rc<RPCClient>, media_manager: Rc<MediaManager>) -> Self {
        Self(Rc::new(RefCell::new(InnerRoom::new(rpc, media_manager))))
    }

    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::downgrade(&self.0))
    }

    /// Subscribes to provided RpcTransport messages.
    pub fn subscribe(&self, rpc: &RPCClient) {
        let inner = Rc::clone(&self.0);

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
    fn new(rpc: Rc<RPCClient>, media_manager: Rc<MediaManager>) -> Self {
        Self {
            rpc,
            media_manager,
            peers: PeerRepository::default(),
            on_local_media: Rc::new(Callback::new()),
        }
    }

    fn on_local_media(&self, f: js_sys::Function) {
        self.on_local_media.set_func(f);
    }

    /// Creates RTCPeerConnection with provided ID.
    fn on_peer_created(
        &mut self,
        peer_id: PeerId,
        sdp_offer: &Option<String>,
        tracks: &[Directional],
    ) {
        let peer = match self.peers.create(peer_id) {
            Ok(peer) => peer,
            Err(err) => {
                err.log_err();
                return;
            },
        };

        let rpc = Rc::clone(&self.rpc);
        peer.on_ice_candidate(move |candidate|{
            rpc.send_command(Command::SetIceCandidate { peer_id, candidate });
        });

        // 1. parse tracks
        // 2. offer/answer
    }

    /// Applies specified SDP Answer to specified RTCPeerConnection.
    fn on_sdp_answer(&mut self, peer_id: PeerId, sdp_answer: &str) {
        if let Some(peer) = self.peers.get_peer(peer_id) {
            spawn_local(peer.set_remote_answer(sdp_answer).or_else(|err| {
                err.log_err();
                Err(())
            }));
        } else {
            // TODO: No peer, whats next?
            WasmErr::from_str(format!("Peer with id {} doesnt exist", peer_id));
        }
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
            // TODO: No peer, whats next?
            WasmErr::from_str(format!("Peer with id {} doesnt exist", peer_id));
        }
    }

    /// Disposes specified RTCPeerConnection's.
    fn on_peers_removed(&mut self, peer_ids: &[PeerId]) {
        peer_ids.iter().for_each(|id| {
            self.peers.remove(*id);
        })
    }
}

impl Drop for InnerRoom {
    fn drop(&mut self) {
        // Drops event handling task.
        self.rpc.unsub();
    }
}
