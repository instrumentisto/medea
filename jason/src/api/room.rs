//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::DerefMut as _,
    rc::{Rc, Weak},
};

use derive_more::Display;
use futures::{
    channel::mpsc, future, stream, Future, FutureExt as _, StreamExt as _,
};
use js_sys::Promise;
use medea_client_api_proto::{
    Command, Direction, Event as RpcEvent, EventHandler, IceCandidate,
    IceConnectionState, IceServer, PeerConnectionState, PeerId, PeerMetrics,
    Track, TrackId, TrackPatch,
};
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, spawn_local};

use crate::{
    media::{MediaStream, MediaStreamSettings},
    peer::{
        MediaConnectionsError, MuteState, PeerError, PeerEvent,
        PeerEventHandler, PeerMediaStream, PeerRepository, RtcStats,
        StableMuteState, TransceiverKind,
    },
    rpc::{
        ClientDisconnect, CloseReason, ReconnectHandle, RpcClient,
        RpcClientError, TransportError,
    },
    utils::{
        console_error, Callback, HandlerDetachedError, JasonError, JsCaused,
        JsError,
    },
};

use super::{connection::Connection, ConnectionHandle};

/// Reason of why [`Room`] has been closed.
///
/// This struct is passed into `on_close_by_server` JS side callback.
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
#[derive(Debug, Display, JsCaused)]
enum RoomError {
    /// Returned if the mandatory callback wasn't set.
    #[display(fmt = "`{}` callback isn't set.", _0)]
    CallbackNotSet(&'static str),

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

    /// Returned if [`MediaStreamTrack`] update failed.
    #[display(fmt = "Failed to update Track with {} ID.", _0)]
    FailedTrackPatch(TrackId),

    /// Typically, returned if [`RoomHandle::mute_audio`]-like functions called
    /// simultaneously.
    #[display(fmt = "Some MediaConnectionsError: {}", _0)]
    MediaConnections(#[js(cause)] MediaConnectionsError),
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
        use PeerError::{
            MediaConnections, MediaManager, RtcPeerConnection, StreamRequest,
        };

        match err {
            MediaConnections(ref e) => match e {
                MediaConnectionsError::InvalidTrackPatch(id) => {
                    Self::FailedTrackPatch(*id)
                }
                _ => Self::InvalidLocalStream(err),
            },
            StreamRequest(_) => Self::InvalidLocalStream(err),
            MediaManager(_) => Self::CouldNotGetLocalMedia(err),
            RtcPeerConnection(_) => Self::PeerConnectionError(err),
        }
    }
}

impl From<MediaConnectionsError> for RoomError {
    #[inline]
    fn from(e: MediaConnectionsError) -> Self {
        Self::MediaConnections(e)
    }
}

/// JS side handle to `Room` where all the media happens.
///
/// Actually, represents a [`Weak`]-based handle to `InnerRoom`.
///
/// For using [`RoomHandle`] on Rust side, consider the `Room`.
// TODO: get rid of this RefCell.
#[wasm_bindgen]
pub struct RoomHandle(Weak<RefCell<InnerRoom>>);

impl RoomHandle {
    /// Implements externally visible `RoomHandle::join`.
    ///
    /// # Errors
    ///
    /// With [`RoomError::CallbackNotSet`] if `on_failed_local_stream` or
    /// `on_connection_loss` callbacks are not set.
    ///
    /// With [`RoomError::CouldNotConnectToServer`] if cannot connect to media
    /// server.
    pub fn inner_join(
        &self,
        token: String,
    ) -> impl Future<Output = Result<(), JasonError>> + 'static {
        let inner = upgrade_or_detached!(self.0, JasonError);

        async move {
            let inner = inner?;

            if !inner.borrow().on_failed_local_stream.is_set() {
                return Err(JasonError::from(tracerr::new!(
                    RoomError::CallbackNotSet("Room.on_failed_local_stream()")
                )));
            }

            if !inner.borrow().on_connection_loss.is_set() {
                return Err(JasonError::from(tracerr::new!(
                    RoomError::CallbackNotSet("Room.on_connection_loss()")
                )));
            }

            inner
                .borrow()
                .rpc
                .connect(token)
                .await
                .map_err(tracerr::map_from_and_wrap!(=> RoomError))?;

            let mut connection_loss_stream =
                inner.borrow().rpc.on_connection_loss();
            let weak_inner = Rc::downgrade(&inner);
            spawn_local(async move {
                while let Some(_) = connection_loss_stream.next().await {
                    match upgrade_or_detached!(weak_inner, JsValue) {
                        Ok(inner) => {
                            let reconnect_handle = ReconnectHandle::new(
                                Rc::downgrade(&inner.borrow().rpc),
                            );
                            inner
                                .borrow()
                                .on_connection_loss
                                .call(reconnect_handle);
                        }
                        Err(e) => {
                            console_error(e);
                            break;
                        }
                    }
                }
            });

            Ok(())
        }
    }

    /// Calls [`InnerRoom::toggle_mute`] until all [`PeerConnection`]s of this
    /// [`Room`] will have same [`MuteState`] as requested.
    async fn toggle_mute(
        &self,
        is_muted: bool,
        kind: TransceiverKind,
    ) -> Result<(), JsValue> {
        let inner = upgrade_or_detached!(self.0, JsValue)?;
        while !inner
            .borrow()
            .is_all_peers_in_mute_state(kind, StableMuteState::from(is_muted))
        {
            let fut = inner.borrow().toggle_mute(is_muted, kind);
            fut.await
                .map_err(tracerr::map_from_and_wrap!(=> RoomError))
                .map_err(|e| JsValue::from(JasonError::from(e)))?;
        }
        Ok(())
    }
}

#[wasm_bindgen]
impl RoomHandle {
    /// Sets callback, which will be invoked on new `Connection` establishing.
    pub fn on_new_connection(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.borrow_mut().on_new_connection.set_func(f))
    }

    /// Sets `on_close` callback, which will be invoked on [`Room`] close,
    /// providing [`RoomCloseReason`].
    pub fn on_close(&mut self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.borrow_mut().on_close.set_func(f))
    }

    /// Sets `on_local_stream` callback. This callback is invoked each time
    /// media acquisition request will resolve successfully. This might
    /// happen in such cases:
    /// 1. Media server initiates media request.
    /// 2. `unmute_audio`/`unmute_video` is called.
    /// 3. [`MediaStreamSettings`] updated via `set_local_media_settings`.
    pub fn on_local_stream(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.borrow_mut().on_local_stream.set_func(f))
    }

    /// Sets `on_failed_local_stream` callback, which will be invoked on local
    /// media acquisition failures.
    pub fn on_failed_local_stream(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.borrow_mut().on_failed_local_stream.set_func(f))
    }

    /// Sets `on_connection_loss` callback, which will be invoked on
    /// [`RpcClient`] connection loss.
    pub fn on_connection_loss(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.borrow_mut().on_connection_loss.set_func(f))
    }

    /// Performs entering to a [`Room`] with the preconfigured authorization
    /// `token` for connection with media server.
    ///
    /// Establishes connection with media server (if it doesn't already exist).
    /// Fails if:
    ///   - `on_failed_local_stream` callback is not set
    ///   - `on_connection_loss` callback is not set
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

    /// Updates this [`Room`]s [`MediaStreamSettings`]. This affects all
    /// [`PeerConnection`]s in this [`Room`]. If [`MediaStreamSettings`] is
    /// configured for some [`Room`], then this [`Room`] can only send
    /// [`MediaStream`] that corresponds to this settings.
    /// [`MediaStreamSettings`] update will change [`MediaStream`] in all
    /// sending peers, so that might cause new [getUserMedia()][1] request.
    ///
    /// Media obtaining/injection errors are fired to `on_failed_local_stream`
    /// callback.
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [1]: https://tinyurl.com/rnxcavf
    pub fn set_local_media_settings(
        &self,
        settings: &MediaStreamSettings,
    ) -> Promise {
        let inner = upgrade_or_detached!(self.0, JasonError);
        let settings = settings.clone();
        future_to_promise(async move {
            let inner = inner?;
            inner.borrow_mut().set_local_media_settings(settings).await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Mutes outbound audio in this [`Room`].
    pub fn mute_audio(&self) -> Promise {
        let this = Self(self.0.clone());
        future_to_promise(async move {
            this.toggle_mute(true, TransceiverKind::Audio).await?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Unmutes outbound audio in this [`Room`].
    pub fn unmute_audio(&self) -> Promise {
        let this = Self(self.0.clone());
        future_to_promise(async move {
            this.toggle_mute(false, TransceiverKind::Audio).await?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Mutes outbound video in this [`Room`].
    pub fn mute_video(&self) -> Promise {
        let this = Self(self.0.clone());
        future_to_promise(async move {
            this.toggle_mute(true, TransceiverKind::Video).await?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Unmutes outbound video in this [`Room`].
    pub fn unmute_video(&self) -> Promise {
        let this = Self(self.0.clone());
        future_to_promise(async move {
            this.toggle_mute(false, TransceiverKind::Video).await?;
            Ok(JsValue::UNDEFINED)
        })
    }
}

/// [`Room`] where all the media happens (manages concrete [`PeerConnection`]s,
/// handles media server events, etc).
///
/// It's used on Rust side and represents a handle to [`InnerRoom`] data.
///
/// For using [`Room`] on JS side, consider the [`RoomHandle`].
///
/// [`PeerConnection`]: crate::peer::PeerConnection
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
                        console_error("Inner Room dropped unexpectedly")
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

    /// Local media stream for injecting into new created [`PeerConnection`]s.
    local_stream_settings: Option<MediaStreamSettings>,

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
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    // TODO: will be extended with some metadata that would allow client to
    //       understand purpose of obtaining this stream.
    on_local_stream: Callback<MediaStream>,

    /// Callback to be invoked when failed obtain [`MediaStream`] from
    /// [`MediaManager`] or failed inject stream into [`PeerConnection`].
    on_failed_local_stream: Rc<Callback<JasonError>>,

    /// Callback to be invoked when [`RpcClient`] loses connection.
    on_connection_loss: Callback<ReconnectHandle>,

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
        peers: Box<dyn PeerRepository>,
        peer_event_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Self {
        Self {
            rpc,
            local_stream_settings: None,
            peers,
            peer_event_sender,
            connections: HashMap::new(),
            on_new_connection: Callback::default(),
            on_local_stream: Callback::default(),
            on_connection_loss: Callback::default(),
            on_failed_local_stream: Rc::new(Callback::default()),
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

    /// Toggles [`Sender`]s [`MuteState`] by provided [`TransceiverKind`] in all
    /// [`PeerConnection`]s in this [`Room`].
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    #[allow(clippy::filter_map)]
    fn toggle_mute(
        &self,
        is_muted: bool,
        kind: TransceiverKind,
    ) -> impl Future<Output = Result<(), Traced<RoomError>>> {
        let peers = self.peers.get_all();
        let rpc = self.rpc.clone();
        async move {
            let peer_mute_state_changed: Vec<_> = peers
                .iter()
                .map(|peer| {
                    let desired_state = StableMuteState::from(is_muted);
                    let tracks_patches: Vec<_> = peer
                        .get_senders(kind)
                        .into_iter()
                        .filter(|sender| match sender.mute_state() {
                            MuteState::Transition(t) => {
                                t.intended() != desired_state
                            }
                            MuteState::Stable(s) => s != desired_state,
                        })
                        .map(|sender| {
                            sender.mute_state_transition_to(desired_state);
                            TrackPatch {
                                id: sender.track_id(),
                                is_muted: Some(is_muted),
                            }
                        })
                        .collect();

                    let wait_state_change: Vec<_> = peer
                        .get_senders(kind)
                        .into_iter()
                        .map(|sender| {
                            sender.when_mute_state_stable(desired_state)
                        })
                        .collect();

                    if !tracks_patches.is_empty() {
                        rpc.send_command(Command::UpdateTracks {
                            peer_id: peer.id(),
                            tracks_patches,
                        });
                    }

                    future::join_all(wait_state_change)
                })
                .collect();

            future::join_all(peer_mute_state_changed)
                .await
                .into_iter()
                .flat_map(IntoIterator::into_iter)
                .collect::<Result<Vec<_>, _>>()
                .map(|_| ())
                .map_err(tracerr::map_from_and_wrap!())
        }
    }

    /// Returns `true` if all [`Sender`]s of this [`Room`] is in provided
    /// [`MuteState`].
    pub fn is_all_peers_in_mute_state(
        &self,
        kind: TransceiverKind,
        mute_state: StableMuteState,
    ) -> bool {
        self.peers
            .get_all()
            .into_iter()
            .find(|p| !p.is_all_senders_in_mute_state(kind, mute_state))
            .is_none()
    }

    /// Updates this [`Room`]s [`MediaStreamSettings`]. This affects all
    /// [`PeerConnection`]s in this [`Room`]. If [`MediaStreamSettings`] is
    /// configured for some [`Room`], then this [`Room`] can only send
    /// [`MediaStream`] that corresponds to this settings.
    /// [`MediaStreamSettings`] update will change [`MediaStream`] in all
    /// sending peers, so that might cause new [getUserMedia()][1] request.
    ///
    /// Media obtaining/injection errors are fired to `on_failed_local_stream`
    /// callback.
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [1]: https://tinyurl.com/rnxcavf
    fn set_local_media_settings(
        &mut self,
        settings: MediaStreamSettings,
    ) -> impl Future<Output = ()> + 'static {
        let peers = self.peers.get_all();
        let settings_clone = settings.clone();
        let error_callback = Rc::clone(&self.on_failed_local_stream);
        self.local_stream_settings.replace(settings);
        async move {
            for peer in peers {
                if let Err(err) = peer
                    .update_local_stream(Some(settings_clone.clone()))
                    .await
                    .map_err(tracerr::map_from_and_wrap!(=> RoomError))
                {
                    error_callback.call(JasonError::from(err));
                }
            }
        }
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
        is_force_relayed: bool,
    ) {
        let peer = match self
            .peers
            .create_peer(
                peer_id,
                ice_servers,
                self.peer_event_sender.clone(),
                is_force_relayed,
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

        let local_stream_constraints = self.local_stream_settings.clone();
        let rpc = Rc::clone(&self.rpc);
        let error_callback = Rc::clone(&self.on_failed_local_stream);
        spawn_local(
            async move {
                match sdp_offer {
                    None => {
                        let sdp_offer = peer
                            .get_offer(tracks, local_stream_constraints)
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
                            .process_offer(
                                offer,
                                tracks,
                                local_stream_constraints,
                            )
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
            .then(|result| async move {
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
        });
    }

    /// Updates [`Track`]s of this [`Room`].
    fn on_tracks_updated(&mut self, peer_id: PeerId, tracks: Vec<TrackPatch>) {
        if let Some(peer) = self.peers.get(peer_id) {
            if let Err(err) = peer.update_senders(tracks) {
                JasonError::from(err).print();
            }
        } else {
            JasonError::from(tracerr::new!(RoomError::NoSuchPeer(peer_id)))
                .print();
        }
    }

    /// Starts SDP renegotiation. Only [ICE restart][1] renegotiation is
    /// supported atm.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcofferoptions-icerestart
    fn on_renegotiation_started(&mut self, peer_id: PeerId, ice_restart: bool) {
        let rpc = Rc::clone(&self.rpc);
        if let Some(peer) = self.peers.get(peer_id) {
            spawn_local(
                async move {
                    let sdp_offer = peer
                        .get_offer(Vec::new(), None, ice_restart)
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
                    Result::<_, Traced<RoomError>>::Ok(())
                }
                .then(|result| async move {
                    if let Err(err) = result {
                        JasonError::from(err.into_parts()).print();
                    };
                }),
            );
        } else {
            // TODO: No peer, whats next?
            JasonError::from(tracerr::new!(RoomError::NoSuchPeer(peer_id)))
                .print();
        }
    }

    /// Sets received SDP offer as found peer's remote description and creates
    /// and sends SDP answer based on offer received.
    fn on_sdp_offer_made(&mut self, peer_id: PeerId, sdp_offer: String) {
        if let Some(peer) = self.peers.get(peer_id) {
            let rpc = Rc::clone(&self.rpc);
            spawn_local(
                async move {
                    let sdp_answer = peer
                        .process_offer(sdp_offer, Vec::new(), None)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    rpc.send_command(Command::MakeSdpAnswer {
                        peer_id,
                        sdp_answer,
                    });
                    Result::<_, Traced<RoomError>>::Ok(())
                }
                .then(|result| async move {
                    if let Err(err) = result {
                        JasonError::from(err.into_parts()).print();
                    };
                }),
            );
        } else {
            // TODO: No peer, whats next?
            JasonError::from(tracerr::new!(RoomError::NoSuchPeer(peer_id)))
                .print();
        }
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
        remote_stream: PeerMediaStream,
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
        self.on_local_stream.call(stream);
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
            metrics: PeerMetrics::IceConnectionState(ice_connection_state),
        });
    }

    /// Handles [`PeerEvent::ConnectionStateChanged`] event and sends new
    /// state to the RPC server.
    fn on_connection_state_changed(
        &mut self,
        peer_id: PeerId,
        peer_connection_state: PeerConnectionState,
    ) {
        self.rpc.send_command(Command::AddPeerConnectionMetrics {
            peer_id,
            metrics: PeerMetrics::PeerConnectionState(peer_connection_state),
        });

        if let PeerConnectionState::Connected = peer_connection_state {
            if let Some(peer) = self.peers.get(peer_id) {
                spawn_local(async move {
                    peer.scrape_and_send_peer_stats().await;
                });
            }
        }
    }

    /// Handles [`PeerEvent::StatsUpdate`] event and sends new stats to the RPC
    /// server.
    fn on_stats_update(&mut self, peer_id: PeerId, stats: RtcStats) {
        self.rpc.send_command(Command::AddPeerConnectionMetrics {
            peer_id,
            metrics: PeerMetrics::RtcStats(stats.0),
        });
    }

    /// Handles [`PeerEvent::NewLocalStreamRequired`] event and updates local
    /// stream of [`PeerConnection`] that sent request.
    fn on_new_local_stream_required(&mut self, peer_id: PeerId) {
        if let Some(peer) = self.peers.get(peer_id) {
            let constraints_clone = self.local_stream_settings.clone();
            let error_callback = Rc::clone(&self.on_failed_local_stream);
            spawn_local(async move {
                if let Err(err) = peer
                    .update_local_stream(constraints_clone)
                    .await
                    .map_err(tracerr::map_from_and_wrap!(=> RoomError))
                {
                    error_callback.call(JasonError::from(err));
                }
            });
        } else {
            JasonError::from(tracerr::new!(RoomError::NoSuchPeer(peer_id)))
                .print();
        }
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
            .map(|result| result.map_err(console_error));
    }
}
