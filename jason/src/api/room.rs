//! Represents Medea room.

use futures::future::Either;
use futures::{
    future::{Future, IntoFuture},
    stream::Stream,
    sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
};
use protocol::{Command, Direction};
use protocol::{Event, IceCandidate, Track};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::api::connection::Connection;
use crate::media::PeerEvent;
use crate::{
    api::ConnectionHandle,
    media::{MediaManager, MediaStreamHandle, PeerId, PeerRepository, Sdp},
    rpc::RPCClient,
    utils::{Callback, WasmErr},
};
use std::collections::HashMap;

#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
/// Room handle accessible from JS.
pub struct RoomHandle(Weak<RefCell<InnerRoom>>);

#[wasm_bindgen]
impl RoomHandle {
    pub fn on_new_connection(&mut self, f: js_sys::Function) {
        if let Some(inner) = self.0.upgrade() {
            inner.borrow_mut().on_new_connection.set_func(f);
        } else {
            let f: Callback<i32, WasmErr> = f.into();
            f.call_err(WasmErr::from_str("Detached state"));
        }
    }
}

/// Room handle being used by Rust external modules.
pub struct Room(Rc<RefCell<InnerRoom>>);

impl Room {
    /// Creates new [`Room`] associating it with provided [`RpcClient`].
    pub fn new(rpc: &Rc<RPCClient>, media_manager: &Rc<MediaManager>) -> Self {
        let (tx, rx) = unbounded();
        let room = Rc::new(RefCell::new(InnerRoom::new(
            Rc::clone(&rpc),
            tx,
            Rc::clone(&media_manager),
        )));

        let inner = Rc::clone(&room);
        let handle_medea_event = rpc
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
                        inner.on_peer_created(peer_id, sdp_offer, tracks);
                    }
                    Event::SdpAnswerMade {
                        peer_id,
                        sdp_answer,
                    } => {
                        inner.on_sdp_answer(peer_id, sdp_answer);
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

        let inner = Rc::clone(&room);
        let handle_peer_event = rx
            .for_each(move |event| {
                let mut inner = inner.borrow_mut();

                match event {
                    PeerEvent::IceCandidateDiscovered { peer_id, ice } => {
                        inner.on_peer_ice_candidate_discovered(peer_id, ice);
                    }
                    PeerEvent::NewRemoteStream { .. } => {}
                }
                Ok(())
            })
            .into_future()
            .then(|_| Ok(()));

        // Spawns Promise in JS, does not provide any handles, so current way to
        // stop this stream is to drop all connected Senders.
        spawn_local(handle_medea_event);
        spawn_local(handle_peer_event);

        Self(room)
    }

    /// Creates new [`RoomHandle`] used by JS side. You can create them as many
    /// as you need.
    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::downgrade(&self.0))
    }
}

/// Actual room. Shared between JS-side handle ([`RoomHandle`]) and Rust-side
/// handle (['Room']). Manages concrete `RTCPeerConnections`, handles Medea
/// events.
struct InnerRoom {
    rpc: Rc<RPCClient>,
    media_manager: Rc<MediaManager>,
    peers: PeerRepository,
    connections: HashMap<u64, Connection>,
    on_new_connection: Rc<Callback<ConnectionHandle, WasmErr>>,
}

impl InnerRoom {
    fn new(
        rpc: Rc<RPCClient>,
        peer_events_sender: UnboundedSender<PeerEvent>,
        media_manager: Rc<MediaManager>,
    ) -> Self {
        Self {
            rpc,
            media_manager,
            peers: PeerRepository::new(peer_events_sender),
            connections: HashMap::new(),
            on_new_connection: Rc::new(Callback::new()),
        }
    }

    fn on_peer_ice_candidate_discovered(
        &mut self,
        peer_id: PeerId,
        candidate: IceCandidate,
    ) {
        self.rpc
            .send_command(Command::SetIceCandidate { peer_id, candidate });
    }

    fn on_new_connection(&self, f: js_sys::Function) {
        self.on_new_connection.set_func(f);
    }

    fn on_peer_created(
        &mut self,
        peer_id: PeerId,
        sdp_offer: Option<String>,
        tracks: Vec<Track>,
    ) {
        let create_connection = |room: &mut InnerRoom, member_id: &u64| {
            if !room.connections.contains_key(member_id) {
                let con = Connection::new(*member_id);
                room.on_new_connection.call_ok(con.new_handle());
                room.connections.insert(*member_id, con);
            }
        };

        // iterate through tracks and create all connections
        for track in tracks.as_slice() {
            match track.direction {
                Direction::Send { ref receivers } => {
                    for receiver in receivers {
                        create_connection(self, receiver);
                    }
                }
                Direction::Recv { ref sender } => {
                    create_connection(self, sender);
                }
            }
        }

        // Create peer
        let peer = match self.peers.create(peer_id) {
            Ok(peer) => peer,
            Err(err) => {
                err.log_err();
                return;
            }
        };

        let rpc = Rc::clone(&self.rpc);
        let peer_rc = Rc::clone(peer);
        // sync provided tracks and process sdp
        spawn_local(peer.update_tracks(tracks, &self.media_manager).and_then(
            move |_| {
                let fut = match sdp_offer {
                    None => Either::A(peer_rc.create_and_set_offer().and_then(
                        move |sdp_offer: String| {
                            rpc.send_command(Command::MakeSdpOffer {
                                peer_id,
                                sdp_offer,
                            });
                            Ok(())
                        },
                    )),
                    Some(offer) => Either::B(
                        peer_rc
                            .set_remote_description(Sdp::Offer(offer))
                            .and_then(move |_| peer_rc.create_and_set_answer())
                            .and_then(move |sdp_answer| {
                                rpc.send_command(Command::MakeSdpAnswer {
                                    peer_id,
                                    sdp_answer,
                                });
                                Ok(())
                            }),
                    ),
                };
                fut.map_err(|err| err.log_err())
            },
        ));
    }

    fn asd(&mut self, id: u64) {
        WasmErr::from_str("aha!").log_err();
    }

    /// Applies specified SDP Answer to specified RTCPeerConnection.
    fn on_sdp_answer(&mut self, peer_id: PeerId, sdp_answer: String) {
        if let Some(peer) = self.peers.get_peer(peer_id) {
            spawn_local(
                peer.set_remote_description(Sdp::Answer(sdp_answer))
                    .or_else(|err| {
                        err.log_err();
                        Err(())
                    }),
            );
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
            spawn_local(
                peer.add_ice_candidate(candidate)
                    .map_err(|err| err.log_err()),
            );
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
