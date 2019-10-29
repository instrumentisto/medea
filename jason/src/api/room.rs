//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::DerefMut as _,
    rc::{Rc, Weak},
};

use anyhow::Error;
use futures::{channel::mpsc, future, stream, FutureExt as _, StreamExt as _};
use js_sys::Promise;
use medea_client_api_proto::{
    Command, Direction, Event as RpcEvent, EventHandler, IceCandidate,
    IceServer, PeerId, Track,
};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, spawn_local};
use web_sys::MediaStream as SysMediaStream;

use crate::{
    peer::{MediaStream, PeerEvent, PeerEventHandler, PeerRepository},
    rpc::RpcClient,
    utils::Callback,
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
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner
            .borrow_mut()
            .on_new_connection
            .set_func(f))
    }

    /// Sets `on_local_stream` callback, which will be invoked once media
    /// acquisition request will resolve successfully. Only invoked if media
    /// request was initiated by rpc server.
    pub fn on_local_stream(&self, f: js_sys::Function) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow_mut().on_local_stream.set_func(f))
    }

    /// Sets `on_failed_local_stream` callback, which will be invoked on local
    /// media acquisition or media injection failures.
    pub fn on_failed_local_stream(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner
            .borrow_mut()
            .on_failed_local_stream
            .set_func(f))
    }

    /// Performs entering to a [`Room`]  with the preconfigured authorization
    /// `token` for connection with media server.
    ///
    /// Establishes connection with media server (if it doesn't already exist).
    /// Fails if unable to connect to media server.
    /// Effectively returns `Result<(), WasmErr>`.
    pub fn join(&self, token: String) -> Promise {
        match map_weak!(self, |inner| Rc::clone(&inner.borrow().rpc)) {
            Ok(rpc) => {
                future_to_promise(async move {
                    rpc.connect(token).await.map(|_| JsValue::NULL).map_err(
                        |err| js_sys::Error::new(&format!("{}", err)).into(),
                    )
                })
            }
            Err(err) => future_to_promise(future::err(err)),
        }
    }

    /// Performs injecting local media stream for all created and new
    /// [`PeerConnection`]s into this [`Room`].
    pub fn inject_local_stream(
        &self,
        stream: SysMediaStream,
    ) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow_mut().inject_local_stream(stream))
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
        enum RoomEvent {
            RpcEvent(RpcEvent),
            PeerEvent(PeerEvent),
        }

        let (tx, peer_events_rx) = mpsc::unbounded();
        let events_stream = rpc.subscribe();
        let room = Rc::new(RefCell::new(InnerRoom::new(rpc, peers, tx)));

        let rpc_events_stream = events_stream.map(RoomEvent::RpcEvent);
        let peer_events_stream = peer_events_rx.map(RoomEvent::PeerEvent);

        let mut events = stream::select(rpc_events_stream, peer_events_stream);

        let inner = Rc::downgrade(&room);
        // Spawns `Promise` in JS, does not provide any handles, so the current
        // way to stop this stream is to drop all connected `Sender`s.
        spawn_local(async move {
            while let Some(event) = events.next().await {
                match inner.upgrade() {
                    None => {
                        // `InnerSession` is gone, which means that `Room` has
                        // been dropped. Not supposed to
                        // happen, actually, since
                        // `InnerSession` should drop its `tx` by unsub from
                        // `RpcClient`.
                        error!("Inner Room dropped unexpectedly")
                    }
                    Some(inner) => {
                        match event {
                            RoomEvent::RpcEvent(event) => {
                                event.dispatch_with(
                                    inner.borrow_mut().deref_mut(),
                                );
                            }
                            RoomEvent::PeerEvent(event) => {
                                event.dispatch_with(
                                    inner.borrow_mut().deref_mut(),
                                );
                            }
                        };
                    }
                }
            }
        });

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
    /// Client to talk with server via Client API RPC.
    rpc: Rc<dyn RpcClient>,

    /// Local media stream for injecting into new created [`PeerConnection`]s.
    local_stream: Option<SysMediaStream>,

    /// [`PeerConnection`] repository.
    peers: Box<dyn PeerRepository>,

    /// Channel for send events produced [`PeerConnection`] to [`Room`].
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Collection of [`Connection`]s with a remote [`Member`]s.
    connections: HashMap<PeerId, Connection>,

    /// Callback from JS side which will be invoked on remote `Member` media
    /// stream arrival.
    on_new_connection: Callback<ConnectionHandle>,

    /// Callback to be invoked when new [`MediaStream`] is acquired providing
    /// its actual underlying [MediaStream][1] object.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    // TODO: will be extended with some metadata that would allow client to
    //       understand purpose of obtaining this stream.
    on_local_stream: Callback<SysMediaStream>,

    /// Callback to be invoked when failed obtain [`MediaStream`] from
    /// [`MediaManager`] or failed inject stream into [`PeerConnection`].
    on_failed_local_stream: Rc<Callback<js_sys::Error>>,

    /// Indicates if outgoing audio is enabled in this [`Room`].
    enabled_audio: bool,

    /// Indicates if outgoing video is enabled in this [`Room`].
    enabled_video: bool,
}

impl InnerRoom {
    /// Creates new [`InnerRoom`].
    #[inline]
    fn new(
        rpc: Rc<dyn RpcClient>,
        peers: Box<dyn PeerRepository>,
        peer_event_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Self {
        Self {
            rpc,
            local_stream: None,
            peers,
            peer_event_sender,
            connections: HashMap::new(),
            on_new_connection: Callback::default(),
            on_local_stream: Callback::default(),
            on_failed_local_stream: Rc::new(Callback::default()),
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
                room.on_new_connection.call(con.new_handle());
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

    /// Inject given local stream into all [`PeerConnection`]s this [`Room`] and
    /// store its for inject into new [`PeerConnection`].
    ///
    /// If inject local stream into existing [`PeerConnection`] failed, invoke
    /// `on_failed_local_stream` callback with failure error.
    fn inject_local_stream(&mut self, stream: SysMediaStream) {
        let peers = self.peers.get_all();
        let injected_stream = Clone::clone(&stream);
        let error_callback = Rc::clone(&self.on_failed_local_stream);
        spawn_local(async move {
            for peer in peers {
                if let Err(err) =
                    peer.inject_local_stream(&injected_stream).await
                {
                    error!(err.to_string());
                    error_callback
                        .call(js_sys::Error::new(&format!("{}", err)));
                }
            }
        });
        self.local_stream.replace(stream);
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
                error!(err.to_string());
                return;
            }
        };

        self.create_connections_from_tracks(&tracks);

        let local_stream = self.local_stream.clone();
        let rpc = Rc::clone(&self.rpc);
        let error_callback = Rc::clone(&self.on_failed_local_stream);
        spawn_local(
            async move {
                match sdp_offer {
                    None => {
                        let sdp_offer = peer
                            .get_offer(tracks, local_stream.as_ref())
                            .await?;
                        let mids = peer.get_mids()?;
                        rpc.send_command(Command::MakeSdpOffer {
                            peer_id,
                            sdp_offer,
                            mids,
                        });
                    }
                    Some(offer) => {
                        let sdp_answer = peer
                            .process_offer(offer, tracks, local_stream.as_ref())
                            .await?;
                        rpc.send_command(Command::MakeSdpAnswer {
                            peer_id,
                            sdp_answer,
                        });
                    }
                };
                Result::<_, Error>::Ok(())
            }
            .then(|result| {
                async move {
                    if let Err(err) = result {
                        error!(err.to_string());
                        error_callback
                            .call(js_sys::Error::new(&format!("{}", err)));
                    };
                }
            }),
        );
    }

    /// Applies specified SDP Answer to a specified [`PeerConnection`].
    fn on_sdp_answer_made(&mut self, peer_id: PeerId, sdp_answer: String) {
        if let Some(peer) = self.peers.get(peer_id) {
            spawn_local(async move {
                if let Err(err) = peer.set_remote_answer(sdp_answer).await {
                    error!(err.to_string());
                }
            });
        } else {
            // TODO: No peer, whats next?
            error!(format!("Peer with id {} doesnt exist", peer_id))
        }
    }

    /// Applies specified [`IceCandidate`] to a specified [`PeerConnection`].
    fn on_ice_candidate_discovered(
        &mut self,
        peer_id: PeerId,
        candidate: IceCandidate,
    ) {
        if let Some(peer) = self.peers.get(peer_id) {
            spawn_local(async move {
                let add = peer
                    .add_ice_candidate(
                        candidate.candidate,
                        candidate.sdp_m_line_index,
                        candidate.sdp_mid,
                    )
                    .await;
                if let Err(err) = add {
                    error!(err.to_string());
                }
            });
        } else {
            // TODO: No peer, whats next?
            error!(format!("Peer with id {} doesnt exist", peer_id))
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
            Some(conn) => conn.on_remote_stream(remote_stream.stream()),
            None => error!("NewRemoteStream from sender without connection"),
        }
    }

    /// Invoke `on_local_stream` [`Room`]'s callback.
    fn on_new_local_stream(&mut self, _: PeerId, stream: MediaStream) {
        self.on_local_stream.call(stream.stream());
    }
}

impl Drop for InnerRoom {
    /// Unsubscribes [`InnerRoom`] from all its subscriptions.
    fn drop(&mut self) {
        self.rpc.unsub();
    }
}
