//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::DerefMut as _,
    rc::{Rc, Weak},
};

use derive_more::Display;
use futures::{
    future::{self, Future as _, IntoFuture},
    stream::Stream as _,
    sync::mpsc::{unbounded, UnboundedSender},
};
use js_sys::Promise;
use medea_client_api_proto::{
    Command, Direction, EventHandler, IceCandidate, IceServer,
    Peer as SnapshotPeer, PeerId, ServerPeerState, Snapshot, Track,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, spawn_local};

use crate::{
    media::MediaStream,
    peer::{
        PeerConnection, PeerEvent, PeerEventHandler, PeerRepository,
        SignalingState,
    },
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
        map_weak!(self, |inner| inner
            .borrow_mut()
            .on_new_connection
            .set_func(f))
    }

    /// Mutes outbound audio in this room.
    pub fn mute_audio(&self) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow_mut().toggle_send_audio(false))
    }

    /// Unmutes outbound audio in this room.
    pub fn unmute_audio(&self) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow_mut().toggle_send_audio(true))
    }

    /// Mutes outbound video in this room.
    pub fn mute_video(&self) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow_mut().toggle_send_video(false))
    }

    /// Unmutes outbound video in this room.
    pub fn unmute_video(&self) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow_mut().toggle_send_video(true))
    }

    /// Returns promise which resolves into [RTCStatsReport][1]
    /// for all [RtcPeerConnection][2]s from this room.
    ///
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/API/RTCStatsReport
    /// [2]: https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection
    pub fn get_stats_for_peer_connections(&self) -> Promise {
        self.0
            .upgrade()
            .unwrap()
            .borrow()
            .get_stats_of_peer_connections()
    }
}

/// [`Room`] where all the media happens (manages concrete [`PeerConnection`]s,
/// handles media server events, etc).
///
/// It's used on Rust side and represents a handle to [`InnerRoom`] data.
///
/// For using [`Room`] on JS side, consider the [`RoomHandle`].
pub struct Room(Rc<RefCell<InnerRoom>>);

impl Room {
    /// Creates new [`Room`] and associates it with a provided [`RpcClient`].
    pub fn new(rpc: Rc<dyn RpcClient>, peers: Box<dyn PeerRepository>) -> Self {
        let (tx, rx) = unbounded();
        let events_stream = rpc.subscribe();

        let room = Rc::new(RefCell::new(InnerRoom::new(rpc, peers, tx)));

        let inner = Rc::downgrade(&room);
        let handle_medea_event = events_stream
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
    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::downgrade(&self.0))
    }
}

/// Errors which can occurs while synchronization of client with server on
/// reconnecting.
#[derive(Debug, Display)]
enum SynchronizationError {
    #[display(
        fmt = "Fatal conflict in server snapshot. Local state: {:?}, server \
               state: {:?}.",
        _0,
        _1
    )]
    FatalConflictInSnapshot(SignalingState, ServerPeerState),
    #[display(
        fmt = "Cannot create Peer in this state. Server state: {:?}.",
        _0
    )]
    CannotCreateNewPeerInThisState(ServerPeerState),
}

/// Actual data of a [`Room`].
///
/// Shared between JS side ([`RoomHandle`]) and Rust side ([`Room`]).
struct InnerRoom {
    rpc: Rc<dyn RpcClient>,
    peers: Box<dyn PeerRepository>,
    peer_event_sender: UnboundedSender<PeerEvent>,
    connections: HashMap<PeerId, Connection>,
    on_new_connection: Rc<Callback2<ConnectionHandle, WasmErr>>,
    enabled_audio: bool,
    enabled_video: bool,
}

impl InnerRoom {
    /// Creates new [`InnerRoom`].
    #[inline]
    fn new(
        rpc: Rc<dyn RpcClient>,
        peers: Box<dyn PeerRepository>,
        peer_event_sender: UnboundedSender<PeerEvent>,
    ) -> Self {
        Self {
            rpc,
            peers,
            peer_event_sender,
            connections: HashMap::new(),
            on_new_connection: Rc::new(Callback2::default()),
            enabled_audio: true,
            enabled_video: true,
        }
    }

    /// Returns promise which resolves into [RTCStatsReport][1]
    /// for all [RtcPeerConnection][2]s from this room.
    ///
    /// [1]: https://developer.mozilla.org/en-US/docs/Web/API/RTCStatsReport
    /// [2]: https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection
    pub fn get_stats_of_peer_connections(&self) -> Promise {
        future_to_promise(
            self.peers
                .get_stats_for_all_peer_connections()
                .map_err(JsValue::from)
                .map(|e| {
                    let js_array = js_sys::Array::new();
                    for id in e {
                        js_array.push(&id);
                    }
                    js_array.into()
                }),
        )
    }

    /// Creates new [`Connection`]s basing on senders and receivers of provided
    /// [`Track`]s.
    // TODO: creates connections based on remote peer_ids atm, should create
    //       connections based on remote member_ids
    fn create_connections_from_tracks(&mut self, tracks: &[Track]) {
        let create_connection = |room: &mut Self, peer_id: &PeerId| {
            if !room.connections.contains_key(peer_id) {
                let con = Connection::new();
                room.on_new_connection.call1(con.new_handle());
                room.connections.insert(*peer_id, con);
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

    /// Toggles a audio send [`Track`]s of all [`PeerConnection`]s what this
    /// [`Room`] manage.
    fn toggle_send_audio(&mut self, enabled: bool) {
        for peer in self.peers.get_all() {
            peer.toggle_send_audio(enabled);
        }
        self.enabled_audio = enabled;
    }

    /// Toggles a video send [`Track`]s of all [`PeerConnection`]s what this
    /// [`Room`] manage.
    fn toggle_send_video(&mut self, enabled: bool) {
        for peer in self.peers.get_all() {
            peer.toggle_send_video(enabled);
        }
        self.enabled_video = enabled;
    }

    /// Resets state of [`InnerRoom`].
    ///
    /// Currently removes all [`Peer`]s and send [`Command::ResetMe`] to the
    /// server.
    fn reset(&mut self) {
        let peers_to_remove =
            self.peers.peers().into_iter().map(|(id, _)| id).collect();
        self.on_peers_removed(peers_to_remove);
        self.rpc.send_command(Command::ResetMe);
    }

    /// Synchronizes local [`PeerConnection`] signaling state with server
    /// [`Peer`] state.
    fn synchronize_peer_signaling_state(
        &mut self,
        local_peer: &Rc<PeerConnection>,
        snapshot_peer: SnapshotPeer,
    ) -> Result<(), SynchronizationError> {
        let peer_id = snapshot_peer.id;

        match snapshot_peer.state {
            ServerPeerState::Stable => match local_peer.signaling_state() {
                SignalingState::Stable => {}
                SignalingState::HaveLocalOffer => {
                    self.on_sdp_answer_made(
                        peer_id,
                        snapshot_peer.sdp_answer.unwrap(),
                    );
                }
                _ => {
                    return Err(SynchronizationError::FatalConflictInSnapshot(
                        local_peer.signaling_state(),
                        snapshot_peer.state,
                    ));
                }
            },
            ServerPeerState::WaitLocalHaveRemoteSdp => {
                match local_peer.signaling_state() {
                    SignalingState::Stable => {
                        let sdp_answer =
                            local_peer.current_local_sdp().unwrap();
                        self.rpc.send_command(Command::MakeSdpAnswer {
                            peer_id,
                            sdp_answer,
                        })
                    }
                    _ => {
                        return Err(
                            SynchronizationError::FatalConflictInSnapshot(
                                local_peer.signaling_state(),
                                snapshot_peer.state,
                            ),
                        )
                    }
                }
            }
            ServerPeerState::WaitLocalSdp => {
                match local_peer.signaling_state() {
                    SignalingState::HaveLocalOffer => {
                        let local_sdp = local_peer.current_local_sdp().unwrap();
                        self.rpc.send_command(Command::MakeSdpOffer {
                            peer_id,
                            sdp_offer: local_sdp,
                            mids: local_peer.get_mids().unwrap(),
                        })
                    }
                    _ => {
                        return Err(
                            SynchronizationError::FatalConflictInSnapshot(
                                local_peer.signaling_state(),
                                snapshot_peer.state,
                            ),
                        )
                    }
                }
            }
            _ => {
                return Err(SynchronizationError::FatalConflictInSnapshot(
                    local_peer.signaling_state(),
                    snapshot_peer.state,
                ))
            }
        }

        // We should add `IceCandidates` only if `PeerConnection` have
        // `remote_description`.
        match local_peer.signaling_state() {
            SignalingState::Stable | SignalingState::HaveRemoteOffer => {
                for ice_candidate in snapshot_peer.ice_candidates {
                    // TODO: This code can be optimized.
                    //       We can add not all IceCandidates, but only absent
                    //       ones, but this will require
                    //       storing of all added IceCandidates
                    //       or their hashes on the Jason side.
                    //       Also we can send from the server only IceCandidates
                    //       hashes in Snapshot
                    (self as &mut dyn EventHandler)
                        .on_ice_candidate_discovered(peer_id, ice_candidate)
                }
            }
            _ => (),
        }

        Ok(())
    }

    /// Creates [`PeerConnection`]s which doesn't exists locally but exists
    /// on server side.
    fn create_peer_from_snapshot(
        &mut self,
        snapshot_peer: SnapshotPeer,
        ice_servers: &[IceServer],
    ) -> Result<(), SynchronizationError> {
        match snapshot_peer.state {
            ServerPeerState::WaitLocalHaveRemoteSdp
            | ServerPeerState::WaitLocalSdp => {
                self.on_peer_created(
                    snapshot_peer.id,
                    snapshot_peer.sdp_offer,
                    snapshot_peer.tracks,
                    ice_servers.to_vec(),
                );
            }
            _ => {
                return Err(
                    SynchronizationError::CannotCreateNewPeerInThisState(
                        snapshot_peer.state,
                    ),
                );
            }
        }
        Ok(())
    }

    /// Remove [`PeerConnection`]s which not presented in server snapshot.
    fn remove_peers_not_presented_in_snapshot(
        &mut self,
        snapshot_peers: &HashMap<PeerId, SnapshotPeer>,
    ) {
        let removed_peers: Vec<_> = self
            .peers
            .peers()
            .keys()
            .filter(|id| !snapshot_peers.contains_key(&id))
            .copied()
            .collect();
        if !removed_peers.is_empty() {
            self.on_peers_removed(removed_peers);
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
        let peer = match self.peers.create_peer(
            peer_id,
            ice_servers,
            self.peer_event_sender.clone(),
            self.enabled_audio,
            self.enabled_video,
        ) {
            Ok(peer) => peer,
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
                    .map(move |sdp_offer| match peer.get_mids() {
                        Ok(mids) => rpc.send_command(Command::MakeSdpOffer {
                            peer_id,
                            sdp_offer,
                            mids,
                        }),
                        Err(err) => err.log_err(),
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
                    candidate.candidate,
                    candidate.sdp_m_line_index,
                    candidate.sdp_mid,
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

    /// Synchronize local state with server state ([`Snapshot`]).
    fn on_restore_state(&mut self, snapshot: Snapshot) {
        self.remove_peers_not_presented_in_snapshot(&snapshot.peers);

        macro_rules! unwrap_sync_err {
            ($call:expr) => {
                if let Err(e) = $call {
                    web_sys::console::error_1(&format!("{}", e).into());
                    self.reset();
                    return;
                }
            };
        }

        for (id, snapshot_peer) in snapshot.peers {
            if let Some(local_peer) = self.peers.get(id) {
                unwrap_sync_err!(self.synchronize_peer_signaling_state(
                    &local_peer,
                    snapshot_peer,
                ));
            } else {
                unwrap_sync_err!(self.create_peer_from_snapshot(
                    snapshot_peer,
                    &snapshot.ice_servers
                ));
            }
        }
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
        sender_id: PeerId,
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
