//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::DerefMut as _,
    rc::{Rc, Weak},
};

use derive_more::Display;
use futures::{channel::mpsc, stream, Future, FutureExt as _, StreamExt as _};
use js_sys::Promise;
use medea_client_api_proto::{
    Command, Direction, Event as RpcEvent, EventHandler, IceCandidate,
    IceConnectionState, IceServer, PeerId, PeerMetrics, Track,
};
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, spawn_local};
use web_sys::MediaStream as SysMediaStream;

use crate::{
    peer::{
        MediaStream, MediaStreamHandle, PeerError, PeerEvent, PeerEventHandler,
        PeerRepository,
    },
    rpc::{RpcClient, RpcClientError, TransportError, WebSocketRpcTransport},
    utils::{Callback, JasonError, JsCaused, JsError},
};

use super::{connection::Connection, ConnectionHandle};

/// Errors that may occur in a [`Room`].
#[derive(Debug, Display, JsCaused)]
enum RoomError {
    /// Returned if the `on_failed_local_stream` callback was not set before
    /// joining the room.
    #[display(fmt = "`on_failed_local_stream` callback is not set")]
    CallbackNotSet,

    /// Returned if unable to init [`RpcTransport`].
    #[display(fmt = "Unable to init RPC transport: {}", _0)]
    InitRpcTransportFailed(#[js(cause)] TransportError),

    /// Returned if [`RpcClient`] was unable to connect to RPC server.
    #[display(fmt = "Unable to connect RPC server: {}", _0)]
    CouldNotConnectToServer(#[js(cause)] RpcClientError),

    /// Returned if the previously added local media stream does not satisfy
    /// the tracks sent from the media server.
    #[display(fmt = "Invalid local stream: {}", _0)]
    InvalidLocalStream(#[js(cause)] PeerError),

    /// Returned if [`PeerConnection`] cannot receive the local stream from
    /// [`MediaManager`].
    #[display(fmt = "Failed to get local stream: {}", _0)]
    CouldNotGetLocalMedia(#[js(cause)] PeerError),

    /// Returned if the requested [`PeerConnection`] is not found.
    #[display(fmt = "Peer with id {} doesnt exist", _0)]
    NoSuchPeer(PeerId),

    /// Returned if an error occurred during the webrtc signaling process
    /// with remote peer.
    #[display(fmt = "Some PeerConnection error: {}", _0)]
    PeerConnectionError(#[js(cause)] PeerError),

    /// Returned if was received event [`PeerEvent::NewRemoteStream`] without
    /// [`Connection`] with remote [`Member`].
    #[display(fmt = "Remote stream from unknown peer")]
    UnknownRemotePeer,
}

impl From<RpcClientError> for RoomError {
    fn from(err: RpcClientError) -> Self {
        Self::CouldNotConnectToServer(err)
    }
}

impl From<TransportError> for RoomError {
    fn from(err: TransportError) -> Self {
        Self::InitRpcTransportFailed(err)
    }
}

impl From<PeerError> for RoomError {
    fn from(err: PeerError) -> Self {
        use PeerError::*;
        use RoomError::*;
        match err {
            MediaConnections(_) | StreamRequest(_) => InvalidLocalStream(err),
            MediaManager(_) => CouldNotGetLocalMedia(err),
            RtcPeerConnection(_) => PeerConnectionError(err),
        }
    }
}

/// JS side handle to `Room` where all the media happens.
///
/// Actually, represents a [`Weak`]-based handle to `InnerRoom`.
///
/// For using [`RoomHandle`] on Rust side, consider the `Room`.
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct RoomHandle(Weak<RefCell<InnerRoom>>);

impl RoomHandle {
    /// Implements externally visible `RoomHandle::join`.
    pub fn inner_join(
        &self,
        token: String,
    ) -> impl Future<Output = Result<(), JasonError>> + 'static {
        let inner: Result<_, JasonError> = map_weak!(self, |inner| inner);

        async move {
            let inner = inner?;

            if !inner.borrow().on_failed_local_stream.is_set() {
                return Err(JasonError::from(tracerr::new!(
                    RoomError::CallbackNotSet
                )));
            }

            let websocket = WebSocketRpcTransport::new(&token)
                .await
                .map_err(tracerr::map_from_and_wrap!(=> RoomError))?;
            inner
                .borrow()
                .rpc
                .connect(Rc::new(websocket))
                .await
                .map_err(tracerr::map_from_and_wrap!(=> RoomError))?;

            Ok(())
        }
    }
}

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
    /// request was initiated by media server.
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

    /// Performs entering to a [`Room`] with the preconfigured authorization
    /// `token` for connection with media server.
    ///
    /// Establishes connection with media server (if it doesn't already exist).
    /// Fails if:
    ///   - `on_failed_local_stream` callback is not set
    ///   - unable to connect to media server.
    ///
    /// Effectively returns `Result<(), JasonError>`.
    pub fn join(&self, token: String) -> Promise {
        future_to_promise(
            self.inner_join(token).map(|result| {
                result.map(|_| JsValue::null()).map_err(Into::into)
            }),
        )
    }

    /// Injects local media stream for all created and new [`PeerConnection`]s
    /// in this [`Room`].
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
                        console_error!("Inner Room dropped unexpectedly")
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
    /// Client to talk with media server via Client API RPC.
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
    on_local_stream: Callback<MediaStreamHandle>,

    /// Callback to be invoked when failed obtain [`MediaStream`] from
    /// [`MediaManager`] or failed inject stream into [`PeerConnection`].
    on_failed_local_stream: Rc<Callback<JasonError>>,

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

    /// Injects given local stream into all [`PeerConnection`]s of this [`Room`]
    /// and stores its for injecting into new [`PeerConnection`]s.
    ///
    /// If injecting fails, then invokes `on_failed_local_stream` callback with
    /// a failure error.
    fn inject_local_stream(&mut self, stream: SysMediaStream) {
        let peers = self.peers.get_all();
        let injected_stream = Clone::clone(&stream);
        let error_callback = Rc::clone(&self.on_failed_local_stream);
        spawn_local(async move {
            for peer in peers {
                if let Err(err) = peer
                    .inject_local_stream(&injected_stream)
                    .await
                    .map_err(tracerr::map_from_and_wrap!(=> RoomError))
                {
                    error_callback.call(JasonError::from(err));
                }
            }
        });
        self.local_stream.replace(stream);
    }
}

/// RPC events handling.
impl EventHandler for InnerRoom {
    type Output = ();

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
        let peer = match self
            .peers
            .create_peer(
                peer_id,
                ice_servers,
                self.peer_event_sender.clone(),
                self.enabled_audio,
                self.enabled_video,
            )
            .map_err(tracerr::map_from_and_wrap!(=> RoomError))
        {
            Ok(peer) => peer,
            Err(err) => {
                JasonError::from(err).print();
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
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                        let mids = peer
                            .get_mids()
                            .map_err(tracerr::map_from_and_wrap!())?;
                        rpc.send_command(Command::MakeSdpOffer {
                            peer_id,
                            sdp_offer,
                            mids,
                        });
                    }
                    Some(offer) => {
                        let sdp_answer = peer
                            .process_offer(offer, tracks, local_stream.as_ref())
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                        rpc.send_command(Command::MakeSdpAnswer {
                            peer_id,
                            sdp_answer,
                        });
                    }
                };
                Result::<_, Traced<RoomError>>::Ok(())
            }
            .then(|result| {
                async move {
                    if let Err(err) = result {
                        let (err, trace) = err.into_parts();
                        match err {
                            RoomError::InvalidLocalStream(_)
                            | RoomError::CouldNotGetLocalMedia(_) => {
                                let e = JasonError::from((err, trace));
                                e.print();
                                error_callback.call(e);
                            }
                            _ => JasonError::from((err, trace)).print(),
                        };
                    };
                }
            }),
        );
    }

    /// Applies specified SDP Answer to a specified [`PeerConnection`].
    fn on_sdp_answer_made(&mut self, peer_id: PeerId, sdp_answer: String) {
        if let Some(peer) = self.peers.get(peer_id) {
            spawn_local(async move {
                if let Err(err) = peer
                    .set_remote_answer(sdp_answer)
                    .await
                    .map_err(tracerr::map_from_and_wrap!(=> RoomError))
                {
                    JasonError::from(err).print()
                }
            });
        } else {
            // TODO: No peer, whats next?
            JasonError::from(tracerr::new!(RoomError::NoSuchPeer(peer_id)))
                .print();
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
                    .await
                    .map_err(tracerr::map_from_and_wrap!(=> RoomError));
                if let Err(err) = add {
                    JasonError::from(err).print();
                }
            });
        } else {
            // TODO: No peer, whats next?
            JasonError::from(tracerr::new!(RoomError::NoSuchPeer(peer_id)))
                .print()
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
    type Output = ();

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
            Some(conn) => conn.on_remote_stream(remote_stream.new_handle()),
            None => {
                JasonError::from(tracerr::new!(RoomError::UnknownRemotePeer))
                    .print()
            }
        }
    }

    /// Invokes `on_local_stream` [`Room`]'s callback.
    fn on_new_local_stream(&mut self, _: PeerId, stream: MediaStream) {
        self.on_local_stream.call(stream.new_handle());
    }

    /// Handles [`PeerEvent::IceConnectionStateChanged`] event and sends new
    /// state to RPC server.
    fn on_ice_connection_state_changed(
        &mut self,
        peer_id: PeerId,
        ice_connection_state: IceConnectionState,
    ) {
        self.rpc.send_command(Command::AddPeerConnectionMetrics {
            peer_id,
            metrics: PeerMetrics::IceConnectionStateChanged(
                ice_connection_state,
            ),
        });
    }
}

impl Drop for InnerRoom {
    /// Unsubscribes [`InnerRoom`] from all its subscriptions.
    fn drop(&mut self) {
        self.rpc.unsub();
    }
}
