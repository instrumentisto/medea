//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::DerefMut,
    rc::{Rc, Weak},
};

use futures::{
    future::{Either, Future as _, IntoFuture},
    stream::Stream as _,
    sync::mpsc::{unbounded, UnboundedSender},
};
use medea_client_api_proto::{
    Command, Direction, EventHandler, IceCandidate, IceServer, Track,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    api::{connection::Connection, ConnectionHandle},
    media::{MediaManager, MediaStream},
    peer::{PeerEvent, PeerEventHandler, PeerId, PeerRepository},
    rpc::RpcClient,
    utils::{Callback2, WasmErr},
};

/// Room handle accessible from JS.
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct RoomHandle(Weak<RefCell<InnerRoom>>);

#[wasm_bindgen]
impl RoomHandle {
    pub fn on_new_connection(
        &mut self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        if let Some(inner) = self.0.upgrade() {
            inner.borrow_mut().on_new_connection.set_func(f);
            Ok(())
        } else {
            Err(WasmErr::build_from_str("Detached state").into())
        }
    }
}

/// Room handle being used by Rust external modules.
pub struct Room(Rc<RefCell<InnerRoom>>);

impl Room {
    /// Creates new [`Room`] associating it with provided [`RpcClient`].
    pub fn new(rpc: &Rc<RpcClient>, media_manager: &Rc<MediaManager>) -> Self {
        let (tx, rx) = unbounded();
        let room = Rc::new(RefCell::new(InnerRoom::new(
            Rc::clone(&rpc),
            tx,
            Rc::clone(&media_manager),
        )));

        let inner = Rc::downgrade(&room);
        let handle_medea_event = rpc
            .subscribe()
            .for_each(move |event| match inner.upgrade() {
                Some(inner) => {
                    event.dispatch_with(inner.borrow_mut().deref_mut());
                    Ok(())
                }
                None => {
                    // InnerSession is gone, which means that Room was
                    // dropped. Not supposed to happen, since InnerSession
                    // should drop its tx by unsubbing from RpcClient.
                    Err(())
                }
            })
            .into_future()
            .then(|_| Ok(()));

        let inner = Rc::downgrade(&room);
        let handle_peer_event = rx
            .for_each(move |event| match inner.upgrade() {
                Some(inner) => {
                    event.dispatch_with(inner.borrow_mut().deref_mut());
                    Ok(())
                }
                None => Err(()),
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

/// Actual room. Manages concrete `RTCPeerConnection`s, handles Medea events.
///
/// Shared between JS-side handle ([`RoomHandle`])
/// and Rust-side handle ([`Room`]).
struct InnerRoom {
    rpc: Rc<RpcClient>,
    media_manager: Rc<MediaManager>,
    peers: PeerRepository,
    connections: HashMap<u64, Connection>,
    on_new_connection: Rc<Callback2<ConnectionHandle, WasmErr>>,
}

impl InnerRoom {
    fn new(
        rpc: Rc<RpcClient>,
        peer_events_sender: UnboundedSender<PeerEvent>,
        media_manager: Rc<MediaManager>,
    ) -> Self {
        Self {
            rpc,
            media_manager,
            peers: PeerRepository::new(peer_events_sender),
            connections: HashMap::new(),
            on_new_connection: Rc::new(Callback2::default()),
        }
    }
}

/// RPC event handlers.
impl EventHandler for InnerRoom {
    /// Creates [`PeerConnection`] with provided ID, all new [`Connections`]
    /// based on provided tracks. If provided sdp offer is Some, then offer is
    /// applied to created peer, and [`Command::MakeSdpAnswer`] is emitted back
    /// to RPC server.
    fn on_peer_created(
        &mut self,
        peer_id: PeerId,
        sdp_offer: Option<String>,
        tracks: Vec<Track>,
        ice_servers: Vec<IceServer>,
    ) {
        let create_connection = |room: &mut Self, member_id: &u64| {
            if !room.connections.contains_key(member_id) {
                let con = Connection::new(*member_id);
                room.on_new_connection.call1(con.new_handle());
                room.connections.insert(*member_id, con);
            }
        };

        // iterate through tracks and create all connections
        for track in tracks.as_slice() {
            match &track.direction {
                Direction::Send { ref receivers } => {
                    for receiver in receivers {
                        create_connection(self, receiver);
                    }
                }
                Direction::Recv { ref sender, .. } => {
                    create_connection(self, sender);
                }
            }
        }

        // Create peer
        let peer = match self.peers.create(peer_id, ice_servers) {
            Ok(peer) => peer,
            Err(err) => {
                err.log_err();
                return;
            }
        };

        let rpc = Rc::clone(&self.rpc);
        let peer_rc = Rc::clone(peer);
        // sync provided tracks and process sdp
        spawn_local(
            peer.update_tracks(tracks, &self.media_manager)
                .and_then(move |_| match sdp_offer {
                    None => Either::A(peer_rc.create_and_set_offer().and_then(
                        move |sdp_offer: String| {
                            rpc.send_command(Command::MakeSdpOffer {
                                peer_id,
                                sdp_offer,
                                mids: peer_rc.get_send_mids().unwrap(),
                            });
                            Ok(())
                        },
                    )),
                    Some(offer) => {
                        let peer_rc1 = Rc::clone(&peer_rc);
                        Either::B(
                            peer_rc
                                .set_remote_offer(&offer)
                                .and_then(move |_| {
                                    peer_rc.create_and_set_answer()
                                })
                                .and_then(move |sdp_answer| {
                                    rpc.send_command(Command::MakeSdpAnswer {
                                        peer_id,
                                        sdp_answer,
                                        mids: peer_rc1.get_send_mids().unwrap(),
                                    });
                                    Ok(())
                                }),
                        )
                    }
                })
                .map_err(|err: WasmErr| err.log_err()),
        );
    }

    /// Applies specified SDP Answer to specified [`PeerConnection`].
    // TODO: pass mids to peer.
    fn on_sdp_answer_made(
        &mut self,
        peer_id: PeerId,
        sdp_answer: String,
        mids: HashMap<u64, String>,
    ) {
        if let Some(peer) = self.peers.get_peer(peer_id) {
            spawn_local(peer.set_remote_answer(&sdp_answer, mids).or_else(
                |err| {
                    err.log_err();
                    Err(())
                },
            ));
        } else {
            // TODO: No peer, whats next?
            WasmErr::build_from_str(format!(
                "Peer with id {} doesnt exist",
                peer_id
            ));
        }
    }

    /// Applies specified ICE Candidate to specified [`PeerConnection`].
    fn on_ice_candidate_discovered(
        &mut self,
        peer_id: PeerId,
        candidate: IceCandidate,
    ) {
        if let Some(peer) = self.peers.get_peer(peer_id) {
            spawn_local(
                peer.add_ice_candidate(
                    &candidate.candidate,
                    candidate.sdp_m_line_index,
                    &candidate.sdp_mid,
                )
                .map_err(|err| err.log_err()),
            );
        } else {
            // TODO: No peer, whats next?
            WasmErr::build_from_str(format!(
                "Peer with id {} doesnt exist",
                peer_id
            ));
        }
    }

    /// Disposes specified RTCPeerConnection's.
    fn on_peers_removed(&mut self, peer_ids: Vec<PeerId>) {
        peer_ids.iter().for_each(|id| {
            self.peers.remove(*id);
        })
    }
}

/// Peers event handlers.
impl PeerEventHandler for InnerRoom {
    /// Handles [`PeerEvent::IceCandidateDiscovered`] event. Sends received
    /// candidate to RPC server.
    fn on_ice_candidate_discovered(
        &mut self,
        peer_id: PeerId,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    ) {
        self.rpc.send_command(Command::SetIceCandidate {
            peer_id,
            candidate: IceCandidate {
                candidate,
                sdp_m_line_index,
                sdp_mid,
            },
        });
    }

    /// Handles [`PeerEvent::NewRemoteStream`] event. Passes received
    /// [`MediaStream`] to related [`Connection`].
    fn on_new_remote_stream(
        &mut self,
        _peer_id: PeerId,
        sender_id: u64,
        remote_stream: MediaStream,
    ) {
        match self.connections.get(&sender_id) {
            None => WasmErr::build_from_str(
                "NewRemoteStream from sender without connection",
            )
            .log_err(),
            Some(connection) => connection.new_remote_stream(&remote_stream),
        }
    }
}

impl Drop for InnerRoom {
    /// Drops event handling task.
    fn drop(&mut self) {
        self.rpc.unsub();
    }
}
