//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::DerefMut as _,
    rc::{Rc, Weak},
};

use derive_more as dm;
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
        MediaStream, PeerError, PeerEvent, PeerEventHandler, PeerRepository,
    },
    rpc::{
        ClientDisconnect, CloseReason, RpcClient, RpcClientError,
        TransportError, WebSocketRpcTransport,
    },
    utils::{Callback, JasonError, JsCaused, JsError},
};

use super::{
    connection::Connection, room_stream::RoomStream, ConnectionHandle,
};

/// Reason of why [`Room`] has been closed.
///
/// This struct is passed into `on_close_by_server` JS side callback.
#[allow(clippy::module_name_repetitions)]
#[wasm_bindgen]
pub struct RoomCloseReason {
    /// Indicator if [`Room`] is closed by server.
    ///
    /// `true` if [`CloseReason::ByServer`].
    is_closed_by_server: bool,

    /// Reason of closing.
    reason: String,

    /// Indicator if closing is considered as error.
    ///
    /// This field may be `true` only on closing by client.
    is_err: bool,
}

impl RoomCloseReason {
    /// Creates new [`ClosedByServerReason`] with provided [`CloseReason`]
    /// converted into [`String`].
    ///
    /// `is_err` may be `true` only on closing by client.
    ///
    /// `is_closed_by_server` is `true` on [`CloseReason::ByServer`].
    pub fn new(reason: CloseReason) -> Self {
        match reason {
            CloseReason::ByServer(reason) => Self {
                reason: reason.to_string(),
                is_closed_by_server: true,
                is_err: false,
            },
            CloseReason::ByClient { reason, is_err } => Self {
                reason: reason.to_string(),
                is_closed_by_server: false,
                is_err,
            },
        }
    }
}

#[wasm_bindgen]
impl RoomCloseReason {
    /// `wasm_bindgen` getter for [`RoomCloseReason::reason`] field.
    pub fn reason(&self) -> String {
        self.reason.clone()
    }

    /// `wasm_bindgen` getter for [`RoomCloseReason::is_closed_by_server`]
    /// field.
    pub fn is_closed_by_server(&self) -> bool {
        self.is_closed_by_server
    }

    /// `wasm_bindgen` getter for [`RoomCloseReason::is_err`] field.
    pub fn is_err(&self) -> bool {
        self.is_err
    }
}

/// Errors that may occur in a [`Room`].
#[derive(Debug, dm::Display, dm::From, JsCaused)]
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

            if !inner.borrow().stream_storage.is_set_on_fail() {
                return Err(tracerr::new!(RoomError::CallbackNotSet).into());
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

    /// Sets `on_close` callback, which will be invoked on [`Room`] close,
    /// providing [`RoomCloseReason`].
    pub fn on_close(&mut self, f: js_sys::Function) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow_mut().on_close.set_func(f))
    }

    /// Sets `on_local_stream` callback, which will be invoked once media
    /// acquisition request will resolve successfully. Only invoked if media
    /// request was initiated by media server.
    pub fn on_local_stream(&self, f: js_sys::Function) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow().stream_storage.on_success(f))
    }

    /// Sets `on_failed_local_stream` callback, which will be invoked on local
    /// media acquisition or media injection failures.
    pub fn on_failed_local_stream(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        map_weak!(self, |inner| inner.borrow().stream_storage.on_fail(f))
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
        map_weak!(self, |inner| inner.borrow().inject_local_stream(stream))
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
    pub fn new(
        rpc: Rc<dyn RpcClient>,
        peers: Box<dyn PeerRepository>,
        stream_source: Rc<RoomStream>,
    ) -> Self {
        enum RoomEvent {
            RpcEvent(RpcEvent),
            PeerEvent(PeerEvent),
        }

        let (tx, peer_events_rx) = mpsc::unbounded();
        let events_stream = rpc.subscribe();
        let room = Rc::new(RefCell::new(InnerRoom::new(
            rpc,
            stream_source,
            peers,
            tx,
        )));

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

    /// Sets `close_reason` of [`InnerRoom`] and consumes [`Room`] pointer.
    ///
    /// Supposed that this function will trigger [`Drop`] implementation of
    /// [`InnerRoom`] and call JS side `on_close` callback with provided
    /// [`CloseReason`].
    ///
    /// Note that this function __doesn't guarantee__ that [`InnerRoom`] will be
    /// dropped because theoretically other pointers to the [`InnerRoom`]
    /// can exist. If you need guarantee of [`InnerRoom`] dropping then you
    /// may check count of pointers to [`InnerRoom`] with
    /// [`Rc::strong_count`].
    pub fn close(self, reason: CloseReason) {
        self.0.borrow_mut().set_close_reason(reason);
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

    /// [`PeerConnection`] repository.
    peers: Box<dyn PeerRepository>,

    /// Channel for send events produced [`PeerConnection`] to [`Room`].
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Collection of [`Connection`]s with a remote [`Member`]s.
    connections: HashMap<PeerId, Connection>,

    /// Callback from JS side which will be invoked on remote `Member` media
    /// stream arrival.
    on_new_connection: Callback<ConnectionHandle>,

    /// Stores the injected local media stream for this [`Room`].
    stream_storage: Rc<RoomStream>,

    /// Indicates if outgoing audio is enabled in this [`Room`].
    enabled_audio: bool,

    /// Indicates if outgoing video is enabled in this [`Room`].
    enabled_video: bool,

    /// JS callback which will be called when this [`Room`] will be closed.
    on_close: Rc<Callback<RoomCloseReason>>,

    /// Reason of [`Room`] closing.
    ///
    /// This [`CloseReason`] will be provided into `on_close` JS callback.
    ///
    /// Note that `None` will be considered as error and `is_err` will be
    /// `true` in [`JsCloseReason`] provided to JS callback.
    close_reason: CloseReason,
}

impl InnerRoom {
    /// Creates new [`InnerRoom`].
    #[inline]
    fn new(
        rpc: Rc<dyn RpcClient>,
        stream_storage: Rc<RoomStream>,
        peers: Box<dyn PeerRepository>,
        peer_event_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Self {
        Self {
            connections: HashMap::new(),
            on_new_connection: Callback::default(),
            peers,
            peer_event_sender,
            rpc,
            stream_storage,
            enabled_audio: true,
            enabled_video: true,
            on_close: Rc::new(Callback::default()),
            close_reason: CloseReason::ByClient {
                reason: ClientDisconnect::RoomUnexpectedlyDropped,
                is_err: true,
            },
        }
    }

    /// Sets `close_reason` of [`InnerRoom`].
    ///
    /// [`Drop`] implementation of [`InnerRoom`] is supposed
    /// to be triggered after this function call.
    fn set_close_reason(&mut self, reason: CloseReason) {
        self.close_reason = reason;
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
    fn inject_local_stream(&self, stream: SysMediaStream) {
        self.stream_storage.store_local_stream(stream);
        let media_source = Rc::clone(&self.stream_storage);
        let peers = self.peers.get_all();

        spawn_local(async move {
            for peer in peers {
                if let Err(err) = peer
                    .update_stream(media_source.as_ref())
                    .await
                    .map_err(tracerr::map_from_and_wrap!(=> RoomError))
                {
                    JasonError::from(err).print();
                }
            }
        });
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

        let rpc = Rc::clone(&self.rpc);
        let media_source = Rc::clone(&self.stream_storage);
        spawn_local(async move {
            if let Err(err) = async move {
                match sdp_offer {
                    None => {
                        let sdp_offer = peer
                            .get_offer(tracks, media_source.as_ref())
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
                            .process_offer(offer, tracks, media_source.as_ref())
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                        rpc.send_command(Command::MakeSdpAnswer {
                            peer_id,
                            sdp_answer,
                        });
                    }
                };
                Ok::<_, Traced<RoomError>>(())
            }
            .await
            {
                JasonError::from(err).print();
            }
        });
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

        if let CloseReason::ByClient { reason, .. } = &self.close_reason {
            self.rpc.set_close_reason(*reason);
        };

        self.on_close
            .call(RoomCloseReason::new(self.close_reason))
            .map(|result| {
                result.map_err(|err| console_error!(err.as_string()))
            });
    }
}
