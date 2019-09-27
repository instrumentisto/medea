//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::DerefMut as _,
    rc::{Rc, Weak},
};

use futures::{
    future::{self, Future as _, IntoFuture},
    stream::select,
    channel::mpsc::{unbounded, UnboundedSender},
};

use futures::{StreamExt as _, FutureExt as _};
use futures::{TryFutureExt};

use medea_client_api_proto::{
    Command, Direction, EventHandler, IceCandidate, IceServer, PeerId, Track, Event as RpcEvent
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    media::MediaStream,
    peer::{PeerEvent, PeerEventHandler, PeerRepository},
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
        let (tx, peer_events_rx) = unbounded();
        let events_stream = rpc.subscribe();
        let room = Rc::new(RefCell::new(InnerRoom::new(rpc, peers, tx)));

        enum RoomEvent {
            RpcEvent(RpcEvent),
            PeerEvent(PeerEvent)
        }

        let inner = Rc::downgrade(&room);


        let rpc_events_stream = futures::StreamExt::map(events_stream, |event| RoomEvent::RpcEvent(event));
//        let peer_events_stream = StreamExt::map() .map(|event| RoomEvent::PeerEvent(event));
//        select(rpc_events_stream, peer_events_stream);
//        events_stream.select(rx);


//        let handle_medea_event = events_stream
//            .for_each(move |event| match inner.upgrade() {
//                Some(inner) => {
//                    event.dispatch_with(inner.borrow_mut().deref_mut());
//                    Ok(())
//                }
//                None => {
//                    // `InnerSession` is gone, which means that `Room` has been
//                    // dropped. Not supposed to happen, since
//                    // `InnerSession` should drop its `tx` by unsub from
//                    // `RpcClient`.
//                    Err(())
//                }
//            })
//            .into_future()
//            .then(|_| Ok(()));
//
//        let inner = Rc::downgrade(&room);
//        let handle_peer_event = rx
//            .for_each(move |event| match inner.upgrade() {
//                Some(inner) => {
//                    event.dispatch_with(inner.borrow_mut().deref_mut());
//                    Ok(())
//                }
//                None => Err(()),
//            })
//            .into_future()
//            .then(|_| Ok(()));
//
//        // Spawns `Promise` in JS, does not provide any handles, so the current
//        // way to stop this stream is to drop all connected `Sender`s.
//        spawn_local(handle_medea_event);
//        spawn_local(handle_peer_event);

        Self(room)
    }

    /// Creates new [`RoomHandle`] used by JS side. You can create them as many
    /// as you need.
    #[inline]
    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::downgrade(&self.0))
    }
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
        spawn_local(async move {
            match sdp_offer {
                None => {
                    let sdp_offer = peer.get_offer(tracks).await?;
                    let mids = peer.get_mids()?;
                    rpc.send_command(Command::MakeSdpOffer {
                        peer_id,
                        sdp_offer,
                        mids,
                    });
                },
                Some(offer) => {
                    peer.process_offer(offer, tracks).await;
                    let sdp_answer = peer.create_and_set_answer().await?;
                    rpc.send_command(Command::MakeSdpAnswer {
                        peer_id,
                        sdp_answer,
                    });
                },
            };
            Result::<_, WasmErr>::Ok(())
        }.then(|result| {
            if let Err(err) = result {
                err.log_err();
            };
            future::ready(())
        }));
    }

    /// Applies specified SDP Answer to a specified [`PeerConnection`].
    fn on_sdp_answer_made(&mut self, peer_id: PeerId, sdp_answer: String) {
        if let Some(peer) = self.peers.get(peer_id) {
            spawn_local(async move {
                if let Err(err) = peer.set_remote_answer(sdp_answer).await {
                    err.log_err();
                }
            });
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
                async move {
                    let add = peer.add_ice_candidate(
                        candidate.candidate,
                        candidate.sdp_m_line_index,
                        candidate.sdp_mid,
                    ).await;
                    if let Err(err) = add {
                        err.log_err();
                    }
                }
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
