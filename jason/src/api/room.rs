//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::DerefMut as _,
    rc::{Rc, Weak},
};

use futures::{
    future::{self, Future as _, IntoFuture},
    stream::Stream as _,
    sync::mpsc::{unbounded, UnboundedSender},
};
use medea_client_api_proto::{
    Command, Direction, EventHandler, IceCandidate, IceServer, PeerState,
    Snapshot, Track,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::RtcSignalingState;

use crate::{
    media::{MediaManager, MediaStream},
    peer::{PeerEvent, PeerEventHandler, PeerId, PeerRepository},
    rpc::RpcClient,
    utils::{Callback2, WasmErr},
};

use super::{connection::Connection, ConnectionHandle};

/// JS side handle to `Room` where all the media happens.
///
/// Actually, represents a [`Weak`]-based handle to `InnerRoom`.
///
/// For using [`RoomHandle`] on Rust side, consider the `Room`.
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct RoomHandle(Weak<RefCell<InnerRoom>>);

#[wasm_bindgen]
impl RoomHandle {
    /// Sets callback, which will be invoked on new `Connection` establishing.
    pub fn on_new_connection(
        &mut self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        self.0
            .upgrade()
            .map(|room| {
                room.borrow_mut().on_new_connection.set_func(f);
            })
            .ok_or_else(|| WasmErr::from("Detached state").into())
    }
}

/// [`Room`] where all the media happens (manages concrete [`PeerConnection`]s,
/// handles media server events, etc).
///
/// It's used on Rust side and represents a handle to [`InnerRoom`] data.
///
/// For using [`Room`] on JS side, consider the [`RoomHandle`].
pub(crate) struct Room(Rc<RefCell<InnerRoom>>);

impl Room {
    /// Creates new [`Room`] and associates it with a provided [`RpcClient`].
    pub(crate) fn new(rpc: &Rc<RpcClient>, mngr: &Rc<MediaManager>) -> Self {
        let (tx, rx) = unbounded();
        let room = Rc::new(RefCell::new(InnerRoom::new(
            Rc::clone(rpc),
            tx,
            Rc::clone(mngr),
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
                    // `InnerSession` is gone, which means that `Room` has been
                    // dropped. Not supposed to happen, actually, since
                    // `InnerSession` should drop its `tx` by unsub from
                    // `RpcClient`.
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

        // Spawns `Promise` in JS, does not provide any handles, so the current
        // way to stop this stream is to drop all connected `Sender`s.
        spawn_local(handle_medea_event);
        spawn_local(handle_peer_event);

        Self(room)
    }

    /// Creates new [`RoomHandle`] used by JS side. You can create them as many
    /// as you need.
    #[inline]
    pub(crate) fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::downgrade(&self.0))
    }
}

/// Actual data of a [`Room`].
///
/// Shared between JS side ([`RoomHandle`]) and Rust side ([`Room`]).
struct InnerRoom {
    rpc: Rc<RpcClient>,
    peers: PeerRepository,
    connections: HashMap<u64, Connection>,
    on_new_connection: Rc<Callback2<ConnectionHandle, WasmErr>>,
}

impl InnerRoom {
    /// Creates new [`InnerRoom`].
    #[inline]
    fn new(
        rpc: Rc<RpcClient>,
        peer_events_sender: UnboundedSender<PeerEvent>,
        media_manager: Rc<MediaManager>,
    ) -> Self {
        Self {
            rpc,
            peers: PeerRepository::new(peer_events_sender, media_manager),
            connections: HashMap::new(),
            on_new_connection: Rc::new(Callback2::default()),
        }
    }

    /// Creates new [`Connection`]s basing on senders and receivers of provided
    /// [`Track`]s.
    fn create_connections_from_tracks(&mut self, tracks: &[Track]) {
        let create_connection = |room: &mut Self, member_id: &u64| {
            if !room.connections.contains_key(member_id) {
                let con = Connection::new(*member_id);
                room.on_new_connection.call1(con.new_handle());
                room.connections.insert(*member_id, con);
            }
        };

        for track in tracks {
            match &track.direction {
                Direction::Send { ref receivers, .. } => {
                    for receiver in receivers {
                        create_connection(self, receiver);
                    }
                }
                Direction::Recv { ref sender, .. } => {
                    create_connection(self, sender);
                }
            }
        }
    }
}

/// RPC events handling.
impl EventHandler for InnerRoom {
    /// Creates [`PeerConnection`] with a provided ID and all the
    /// [`Connection`]s basing on provided [`Track`]s.
    ///
    /// If provided `sdp_offer` is `Some`, then offer is applied to a created
    /// peer, and [`Command::MakeSdpAnswer`] is emitted back to the RPC server.
    fn on_peer_created(
        &mut self,
        peer_id: PeerId,
        sdp_offer: Option<String>,
        tracks: Vec<Track>,
        ice_servers: Vec<IceServer>,
    ) {
        let peer = match self.peers.create(peer_id, ice_servers) {
            Ok(peer) => Rc::clone(peer),
            Err(err) => {
                err.log_err();
                return;
            }
        };

        self.create_connections_from_tracks(&tracks);

        let rpc = Rc::clone(&self.rpc);
        let fut = match sdp_offer {
            // offerer
            None => future::Either::A(
                peer.get_offer(tracks)
                    .map(move |sdp_offer| {
                        rpc.send_command(Command::MakeSdpOffer {
                            peer_id,
                            sdp_offer,
                            mids: peer.get_mids().unwrap(),
                        })
                    })
                    .map_err(|err| err.log_err()),
            ),
            Some(offer) => {
                // answerer
                future::Either::B(
                    peer.process_offer(offer, tracks)
                        .and_then(move |_| peer.create_and_set_answer())
                        .map(move |sdp_answer| {
                            rpc.send_command(Command::MakeSdpAnswer {
                                peer_id,
                                sdp_answer,
                            })
                        })
                        .map_err(|err| err.log_err()),
                )
            }
        };

        spawn_local(fut);
    }

    /// Applies specified SDP Answer to a specified [`PeerConnection`].
    fn on_sdp_answer_made(&mut self, peer_id: PeerId, sdp_answer: String) {
        if let Some(peer) = self.peers.get(peer_id) {
            spawn_local(peer.set_remote_answer(sdp_answer).or_else(|err| {
                err.log_err();
                Err(())
            }));
        } else {
            // TODO: No peer, whats next?
            WasmErr::from(format!("Peer with id {} doesnt exist", peer_id))
                .log_err()
        }
    }

    /// Applies specified [`IceCandidate`] to a specified [`PeerConnection`].
    fn on_ice_candidate_discovered(
        &mut self,
        peer_id: PeerId,
        candidate: IceCandidate,
    ) {
        if let Some(peer) = self.peers.get(peer_id) {
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
            WasmErr::from(format!("Peer with id {} doesnt exist", peer_id))
                .log_err()
        }
    }

    /// Disposes specified [`PeerConnection`]s.
    fn on_peers_removed(&mut self, peer_ids: Vec<PeerId>) {
        // TODO: drop connections
        peer_ids.iter().for_each(|id| {
            self.peers.remove(*id);
        })
    }

    fn on_restore_state(&mut self, snapshot: Snapshot) {
        for (id, peer) in snapshot.peers {
            let local_peer = if let Some(local_peer) = self.peers.get(id) {
                local_peer
            } else {
                match peer.state {
                    PeerState::WaitLocalHaveRemoteSdp
                    | PeerState::WaitLocalSdp => {
                        self.on_peer_created(
                            peer.id,
                            peer.sdp_offer,
                            peer.tracks,
                            snapshot.ice_servers.clone(),
                        );
                    }
                    _ => {
                        // TODO: In principle, PeerCreated cannot come in any
                        // state anymore.
                        unimplemented!()
                    }
                }
                continue;
            };

            match peer.state {
                PeerState::Stable => {
                    match local_peer.signaling_state() {
                        RtcSignalingState::Stable => {
                            let remote_desc =
                                local_peer.current_remote_description();
                            let local_desc =
                                local_peer.current_local_description();
                            if !(remote_desc.is_some() && local_desc.is_some())
                            {
                                // TODO: return error because this is not
                                // possible
                            }
                        }
                        RtcSignalingState::HaveLocalOffer => {
                            self.on_sdp_answer_made(
                                peer.id,
                                peer.sdp_answer.unwrap(),
                            );
                        }
                        _ => {
                            // TODO: return error because this is not possible
                        }
                    }
                }
                _ => {
                    // TODO: unreachable??
                    unreachable!()
                }
            }
        }

        //        for (id, peer) in &self.peers {
        //            if let None = snapshot.peers.get(id) {
        //                unimplemented!()
        //                // TODO: remote peer
        //            }
        //        }
    }
}

/// [`PeerEvent`]s handling.
impl PeerEventHandler for InnerRoom {
    /// Handles [`PeerEvent::IceCandidateDiscovered`] event and sends received
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

    /// Handles [`PeerEvent::NewRemoteStream`] event and passes received
    /// [`MediaStream`] to the related [`Connection`].
    fn on_new_remote_stream(
        &mut self,
        _: PeerId,
        sender_id: u64,
        remote_stream: MediaStream,
    ) {
        match self.connections.get(&sender_id) {
            Some(conn) => conn.on_remote_stream(&remote_stream),
            None => {
                WasmErr::from("NewRemoteStream from sender without connection")
                    .log_err()
            }
        }
    }
}

impl Drop for InnerRoom {
    /// Unsubscribes [`InnerRoom`] from all its subscriptions.
    fn drop(&mut self) {
        self.rpc.unsub();
    }
}
