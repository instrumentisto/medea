//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::Deref as _,
    rc::{Rc, Weak},
};

use async_trait::async_trait;
use derive_more::Display;
use futures::{channel::mpsc, future, future::Either, StreamExt as _};
use js_sys::Promise;
use medea_client_api_proto::{
    Command, Direction, Event as RpcEvent, EventHandler, IceCandidate,
    IceConnectionState, IceServer, NegotiationRole, PeerConnectionState,
    PeerId, PeerMetrics, Track, TrackId, TrackPatch, TrackUpdate,
};
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, spawn_local};

use crate::{
    media::{
        LocalStreamConstraints, MediaStream, MediaStreamSettings,
        MediaStreamTrack,
    },
    peer::{
        MediaConnectionsError, MuteState, PeerConnection, PeerError, PeerEvent,
        PeerEventHandler, PeerRepository, RtcStats, Sender, StableMuteState,
        TransceiverKind,
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
pub struct RoomHandle(Weak<InnerRoom>);

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
    pub async fn inner_join(&self, token: String) -> Result<(), JasonError> {
        let inner = upgrade_or_detached!(self.0, JasonError)?;

        if !inner.on_failed_local_stream.is_set() {
            return Err(JasonError::from(tracerr::new!(
                RoomError::CallbackNotSet("Room.on_failed_local_stream()")
            )));
        }

        if !inner.on_connection_loss.is_set() {
            return Err(JasonError::from(tracerr::new!(
                RoomError::CallbackNotSet("Room.on_connection_loss()")
            )));
        }

        inner
            .rpc
            .connect(token)
            .await
            .map_err(tracerr::map_from_and_wrap!( => RoomError))?;

        let mut connection_loss_stream = inner.rpc.on_connection_loss();
        let weak_inner = Rc::downgrade(&inner);
        spawn_local(async move {
            while connection_loss_stream.next().await.is_some() {
                match upgrade_or_detached!(weak_inner, JsValue) {
                    Ok(inner) => {
                        let reconnect_handle =
                            ReconnectHandle::new(Rc::downgrade(&inner.rpc));
                        inner.on_connection_loss.call(reconnect_handle);
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

    /// Calls [`InnerRoom::toggle_mute`] until all [`PeerConnection`]s of this
    /// [`Room`] will have same [`MuteState`] as requested.
    async fn toggle_mute(
        &self,
        is_muted: bool,
        kind: TransceiverKind,
    ) -> Result<(), JasonError> {
        let inner = upgrade_or_detached!(self.0, JasonError)?;
        inner.local_stream_settings.toggle_enable(!is_muted, kind);
        while !inner
            .is_all_peers_in_mute_state(kind, StableMuteState::from(is_muted))
        {
            inner
                .toggle_mute(is_muted, kind)
                .await
                .map_err::<Traced<RoomError>, _>(|e| {
                    inner.local_stream_settings.toggle_enable(is_muted, kind);
                    tracerr::new!(e)
                })?;
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
            .map(|inner| inner.on_new_connection.set_func(f))
    }

    /// Sets `on_close` callback, which will be invoked on [`Room`] close,
    /// providing [`RoomCloseReason`].
    pub fn on_close(&mut self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| inner.on_close.set_func(f))
    }

    /// Sets `on_local_stream` callback. This callback is invoked each time
    /// media acquisition request will resolve successfully. This might
    /// happen in such cases:
    /// 1. Media server initiates media request.
    /// 2. `unmute_audio`/`unmute_video` is called.
    /// 3. [`MediaStreamSettings`] updated via `set_local_media_settings`.
    pub fn on_local_stream(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.on_local_stream.set_func(f))
    }

    /// Sets `on_failed_local_stream` callback, which will be invoked on local
    /// media acquisition failures.
    pub fn on_failed_local_stream(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.on_failed_local_stream.set_func(f))
    }

    /// Sets `on_connection_loss` callback, which will be invoked on
    /// [`RpcClient`] connection loss.
    pub fn on_connection_loss(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.on_connection_loss.set_func(f))
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
        let this = Self(self.0.clone());
        future_to_promise(async move {
            this.inner_join(token).await?;
            Ok(JsValue::undefined())
        })
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
            inner?.set_local_media_settings(settings).await;
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
pub struct Room(Rc<InnerRoom>);

impl Room {
    /// Creates new [`Room`] and associates it with a provided [`RpcClient`].
    #[allow(clippy::mut_mut)]
    pub fn new(rpc: Rc<dyn RpcClient>, peers: Box<dyn PeerRepository>) -> Self {
        enum RoomEvent {
            RpcEvent(RpcEvent),
            PeerEvent(PeerEvent),
            RpcClientLostConnection,
            RpcClientReconnected,
        }

        let (tx, peer_events_rx) = mpsc::unbounded();

        let mut rpc_events_stream =
            rpc.subscribe().map(RoomEvent::RpcEvent).fuse();
        let mut peer_events_stream =
            peer_events_rx.map(RoomEvent::PeerEvent).fuse();
        let mut rpc_connection_lost = rpc
            .on_connection_loss()
            .map(|_| RoomEvent::RpcClientLostConnection)
            .fuse();
        let mut rpc_client_reconnected = rpc
            .on_reconnected()
            .map(|_| RoomEvent::RpcClientReconnected)
            .fuse();

        let room = Rc::new(InnerRoom::new(
            rpc,
            peers,
            tx,
            LocalStreamConstraints::default(),
        ));
        let inner = Rc::downgrade(&room);

        spawn_local(async move {
            loop {
                let event: RoomEvent = futures::select! {
                    event = rpc_events_stream.select_next_some() => event,
                    event = peer_events_stream.select_next_some() => event,
                    event = rpc_connection_lost.select_next_some() => event,
                    event = rpc_client_reconnected.select_next_some() => event,
                    complete => break,
                };

                match inner.upgrade() {
                    None => {
                        console_error("Inner Room dropped unexpectedly");
                        break;
                    }
                    Some(inner) => {
                        match event {
                            RoomEvent::RpcEvent(event) => {
                                if let Err(err) =
                                    event.dispatch_with(inner.deref()).await
                                {
                                    let (err, trace) = err.into_parts();
                                    match err {
                                        RoomError::InvalidLocalStream(_)
                                        | RoomError::CouldNotGetLocalMedia(_) =>
                                        {
                                            let e =
                                                JasonError::from((err, trace));
                                            e.print();
                                            inner
                                                .on_failed_local_stream
                                                .call(e);
                                        }
                                        _ => JasonError::from((err, trace))
                                            .print(),
                                    };
                                };
                            }
                            RoomEvent::PeerEvent(event) => {
                                if let Err(err) =
                                    event.dispatch_with(inner.deref()).await
                                {
                                    JasonError::from(err).print();
                                };
                            }
                            RoomEvent::RpcClientLostConnection => {
                                inner.handle_rpc_connection_lost();
                            }
                            RoomEvent::RpcClientReconnected => {
                                inner.handle_rpc_connection_recovered();
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
        self.0.set_close_reason(reason);
    }

    /// Creates new [`RoomHandle`] used by JS side. You can create them as many
    /// as you need.
    #[inline]
    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::downgrade(&self.0))
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    ///
    /// Used to inspect [`Room`]s inner state in integration tests.
    #[cfg(feature = "mockable")]
    pub fn get_peer_by_id(
        &self,
        peer_id: PeerId,
    ) -> Option<Rc<PeerConnection>> {
        self.0.peers.get(peer_id)
    }
}

/// Actual data of a [`Room`].
///
/// Shared between JS side ([`RoomHandle`]) and Rust side ([`Room`]).
struct InnerRoom {
    /// Client to talk with media server via Client API RPC.
    rpc: Rc<dyn RpcClient>,

    /// Local media stream for injecting into new created [`PeerConnection`]s.
    local_stream_settings: LocalStreamConstraints,

    /// [`PeerConnection`] repository.
    peers: Box<dyn PeerRepository>,

    /// Channel for send events produced [`PeerConnection`] to [`Room`].
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Collection of [`Connection`]s with a remote [`Member`]s.
    connections: RefCell<HashMap<PeerId, Connection>>,

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
    close_reason: RefCell<CloseReason>,
}

impl InnerRoom {
    /// Creates new [`InnerRoom`].
    #[inline]
    fn new(
        rpc: Rc<dyn RpcClient>,
        peers: Box<dyn PeerRepository>,
        peer_event_sender: mpsc::UnboundedSender<PeerEvent>,
        local_stream_settings: LocalStreamConstraints,
    ) -> Self {
        Self {
            rpc,
            local_stream_settings,
            peers,
            peer_event_sender,
            connections: RefCell::new(HashMap::new()),
            on_new_connection: Callback::default(),
            on_local_stream: Callback::default(),
            on_connection_loss: Callback::default(),
            on_failed_local_stream: Rc::new(Callback::default()),
            on_close: Rc::new(Callback::default()),
            close_reason: RefCell::new(CloseReason::ByClient {
                reason: ClientDisconnect::RoomUnexpectedlyDropped,
                is_err: true,
            }),
        }
    }

    /// Sets `close_reason` of [`InnerRoom`].
    ///
    /// [`Drop`] implementation of [`InnerRoom`] is supposed
    /// to be triggered after this function call.
    fn set_close_reason(&self, reason: CloseReason) {
        self.close_reason.replace(reason);
    }

    /// Creates new [`Connection`]s basing on senders and receivers of provided
    /// [`Track`]s.
    // TODO: creates connections based on remote peer_ids atm, should create
    //       connections based on remote member_ids
    fn create_connections_from_tracks(&self, tracks: &[Track]) {
        let create_connection = |room: &Self, peer_id: &PeerId| {
            let is_new = !room.connections.borrow().contains_key(peer_id);
            if is_new {
                let con = Connection::new();
                room.on_new_connection.call(con.new_handle());
                room.connections.borrow_mut().insert(*peer_id, con);
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
    async fn toggle_mute(
        &self,
        is_muted: bool,
        kind: TransceiverKind,
    ) -> Result<(), Traced<RoomError>> {
        let peer_mute_state_changed: Vec<_> = self
            .peers
            .get_all()
            .iter()
            .map(|peer| {
                let desired_state = StableMuteState::from(is_muted);
                let senders = peer.get_senders(kind);

                let senders_to_mute = senders.into_iter().filter(|sender| {
                    match sender.mute_state() {
                        MuteState::Transition(t) => {
                            t.intended() != desired_state
                        }
                        MuteState::Stable(s) => s != desired_state,
                    }
                });

                let mut processed_senders: Vec<Rc<Sender>> = Vec::new();
                let mut tracks_patches = Vec::new();
                for sender in senders_to_mute {
                    if let Err(e) =
                        sender.mute_state_transition_to(desired_state)
                    {
                        for processed_sender in processed_senders {
                            processed_sender.cancel_transition();
                        }
                        return Either::Left(future::err(tracerr::new!(e)));
                    }
                    tracks_patches.push(TrackPatch {
                        id: sender.track_id(),
                        is_muted: Some(is_muted),
                    });
                    processed_senders.push(sender);
                }

                let wait_state_change: Vec<_> = peer
                    .get_senders(kind)
                    .into_iter()
                    .map(|sender| sender.when_mute_state_stable(desired_state))
                    .collect();

                if !tracks_patches.is_empty() {
                    self.rpc.send_command(Command::UpdateTracks {
                        peer_id: peer.id(),
                        tracks_patches,
                    });
                }

                Either::Right(future::try_join_all(wait_state_change))
            })
            .collect();

        future::try_join_all(peer_mute_state_changed)
            .await
            .map_err(tracerr::map_from_and_wrap!())?;
        Ok(())
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
    async fn set_local_media_settings(&self, settings: MediaStreamSettings) {
        self.local_stream_settings.constrain(settings);
        for peer in self.peers.get_all() {
            if let Err(err) = peer
                .update_local_stream()
                .await
                .map_err(tracerr::map_from_and_wrap!(=> RoomError))
            {
                self.on_failed_local_stream.call(JasonError::from(err));
            }
        }
    }

    /// Stops state transition timers in all [`PeerConnection`]'s in this
    /// [`Room`].
    fn handle_rpc_connection_lost(&self) {
        for peer in self.peers.get_all() {
            peer.stop_state_transitions_timers();
        }
    }

    /// Resets state transition timers in all [`PeerConnection`]'s in this
    /// [`Room`].
    fn handle_rpc_connection_recovered(&self) {
        for peer in self.peers.get_all() {
            peer.reset_state_transitions_timers();
        }
    }

    /// Creates new [`Sender`]s and [`Receiver`]s for each new [`Track`] in
    /// provided [`PeerConnection`]. Negotiates [`PeerConnection`] if provided
    /// `negotiation_role` is `Some`.
    async fn create_tracks_and_maybe_negotiate(
        &self,
        peer: Rc<PeerConnection>,
        tracks: Vec<Track>,
        negotiation_role: Option<NegotiationRole>,
    ) -> Result<(), Traced<RoomError>> {
        match negotiation_role {
            None => {
                peer.create_tracks(tracks)
                    .map_err(tracerr::map_from_and_wrap!())?;
            }
            Some(NegotiationRole::Offerer) => {
                let sdp_offer = peer
                    .get_offer(tracks)
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;
                let mids =
                    peer.get_mids().map_err(tracerr::map_from_and_wrap!())?;
                let senders_statuses = peer.get_senders_statuses();
                self.rpc.send_command(Command::MakeSdpOffer {
                    peer_id: peer.id(),
                    sdp_offer,
                    senders_statuses,
                    mids,
                });
            }
            Some(NegotiationRole::Answerer(offer)) => {
                let sdp_answer = peer
                    .process_offer(offer, tracks)
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;
                let senders_statuses = peer.get_senders_statuses();
                self.rpc.send_command(Command::MakeSdpAnswer {
                    peer_id: peer.id(),
                    sdp_answer,
                    senders_statuses,
                });
            }
        };
        Ok(())
    }
}

/// RPC events handling.
#[async_trait(?Send)]
impl EventHandler for InnerRoom {
    type Output = Result<(), Traced<RoomError>>;

    /// Creates [`PeerConnection`] with a provided ID and all the
    /// [`Connection`]s basing on provided [`Track`]s.
    ///
    /// If provided `sdp_offer` is `Some`, then offer is applied to a created
    /// peer, and [`Command::MakeSdpAnswer`] is emitted back to the RPC server.
    async fn on_peer_created(
        &self,
        peer_id: PeerId,
        negotiation_role: NegotiationRole,
        tracks: Vec<Track>,
        ice_servers: Vec<IceServer>,
        is_force_relayed: bool,
    ) -> Result<(), Traced<RoomError>> {
        let peer = self
            .peers
            .create_peer(
                peer_id,
                ice_servers,
                self.peer_event_sender.clone(),
                is_force_relayed,
                self.local_stream_settings.clone(),
            )
            .map_err(tracerr::map_from_and_wrap!())?;

        self.create_connections_from_tracks(&tracks);
        self.create_tracks_and_maybe_negotiate(
            peer,
            tracks,
            Some(negotiation_role),
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())?;
        Ok(())
    }

    /// Applies specified SDP Answer to a specified [`PeerConnection`].
    async fn on_sdp_answer_made(
        &self,
        peer_id: PeerId,
        sdp_answer: String,
    ) -> Result<(), Traced<RoomError>> {
        let peer = self
            .peers
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(RoomError::NoSuchPeer(peer_id)))?;
        peer.set_remote_answer(sdp_answer)
            .await
            .map_err(tracerr::map_from_and_wrap!())
    }

    /// Applies specified [`IceCandidate`] to a specified [`PeerConnection`].
    async fn on_ice_candidate_discovered(
        &self,
        peer_id: PeerId,
        candidate: IceCandidate,
    ) -> Result<(), Traced<RoomError>> {
        let peer = self
            .peers
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(RoomError::NoSuchPeer(peer_id)))?;

        peer.add_ice_candidate(
            candidate.candidate,
            candidate.sdp_m_line_index,
            candidate.sdp_mid,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Disposes specified [`PeerConnection`]s.
    async fn on_peers_removed(
        &self,
        peer_ids: Vec<PeerId>,
    ) -> Result<(), Traced<RoomError>> {
        // TODO: drop connections
        peer_ids.iter().for_each(|id| {
            self.peers.remove(*id);
        });
        Ok(())
    }

    /// Creates new `Track`s, updates existing [`Sender`]s/[`Receiver`]s with
    /// [`TrackUpdate`]s.
    ///
    /// Will start renegotiation process if `Some` [`NegotiationRole`] is
    /// provided.
    async fn on_tracks_applied(
        &self,
        peer_id: PeerId,
        updates: Vec<TrackUpdate>,
        negotiation_role: Option<NegotiationRole>,
    ) -> Result<(), Traced<RoomError>> {
        let peer = self
            .peers
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(RoomError::NoSuchPeer(peer_id)))?;
        let mut new_tracks = Vec::new();
        let mut patches = Vec::new();

        for update in updates {
            match update {
                TrackUpdate::Added(track) => {
                    new_tracks.push(track);
                }
                TrackUpdate::Updated(track_patch) => {
                    patches.push(track_patch);
                }
            }
        }
        peer.update_senders(patches)
            .map_err(tracerr::map_from_and_wrap!())?;
        self.create_tracks_and_maybe_negotiate(
            peer,
            new_tracks,
            negotiation_role,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())?;
        Ok(())
    }
}

/// [`PeerEvent`]s handling.
#[async_trait(?Send)]
impl PeerEventHandler for InnerRoom {
    type Output = Result<(), Traced<RoomError>>;

    /// Handles [`PeerEvent::IceCandidateDiscovered`] event and sends received
    /// candidate to RPC server.
    async fn on_ice_candidate_discovered(
        &self,
        peer_id: PeerId,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    ) -> Result<(), Traced<RoomError>> {
        self.rpc.send_command(Command::SetIceCandidate {
            peer_id,
            candidate: IceCandidate {
                candidate,
                sdp_m_line_index,
                sdp_mid,
            },
        });
        Ok(())
    }

    /// Handles [`PeerEvent::NewRemoteTrack`] event and passes received
    /// [`MediaStreamTrack`] to the related [`Connection`].
    async fn on_new_remote_track(
        &self,
        _: PeerId,
        sender_id: PeerId,
        track_id: TrackId,
        track: MediaStreamTrack,
    ) -> Result<(), Traced<RoomError>> {
        let connections_ref = self.connections.borrow();
        let conn = connections_ref
            .get(&sender_id)
            .ok_or_else(|| tracerr::new!(RoomError::UnknownRemotePeer))?;
        conn.add_remote_track(track_id, track);
        Ok(())
    }

    /// Invokes `on_local_stream` [`Room`]'s callback.
    async fn on_new_local_stream(
        &self,
        _: PeerId,
        stream: MediaStream,
    ) -> Result<(), Traced<RoomError>> {
        self.on_local_stream.call(stream);
        Ok(())
    }

    /// Handles [`PeerEvent::IceConnectionStateChanged`] event and sends new
    /// state to RPC server.
    async fn on_ice_connection_state_changed(
        &self,
        peer_id: PeerId,
        ice_connection_state: IceConnectionState,
    ) -> Result<(), Traced<RoomError>> {
        self.rpc.send_command(Command::AddPeerConnectionMetrics {
            peer_id,
            metrics: PeerMetrics::IceConnectionState(ice_connection_state),
        });
        Ok(())
    }

    /// Handles [`PeerEvent::ConnectionStateChanged`] event and sends new
    /// state to the RPC server.
    async fn on_connection_state_changed(
        &self,
        peer_id: PeerId,
        peer_connection_state: PeerConnectionState,
    ) -> Result<(), Traced<RoomError>> {
        self.rpc.send_command(Command::AddPeerConnectionMetrics {
            peer_id,
            metrics: PeerMetrics::PeerConnectionState(peer_connection_state),
        });

        if let PeerConnectionState::Connected = peer_connection_state {
            if let Some(peer) = self.peers.get(peer_id) {
                peer.scrape_and_send_peer_stats().await;
            }
        };
        Ok(())
    }

    /// Handles [`PeerEvent::StatsUpdate`] event and sends new stats to the RPC
    /// server.
    async fn on_stats_update(
        &self,
        peer_id: PeerId,
        stats: RtcStats,
    ) -> Result<(), Traced<RoomError>> {
        self.rpc.send_command(Command::AddPeerConnectionMetrics {
            peer_id,
            metrics: PeerMetrics::RtcStats(stats.0),
        });
        Ok(())
    }

    /// Handles [`PeerEvent::NewLocalStreamRequired`] event and updates local
    /// stream of [`PeerConnection`] that sent request.
    async fn on_new_local_stream_required(
        &self,
        peer_id: PeerId,
    ) -> Result<(), Traced<RoomError>> {
        let peer = self
            .peers
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(RoomError::NoSuchPeer(peer_id)))?;
        if let Err(err) = peer
            .update_local_stream()
            .await
            .map_err(tracerr::map_from_and_wrap!(=> RoomError))
        {
            self.on_failed_local_stream.call(JasonError::from(err));
        };
        Ok(())
    }
}

impl Drop for InnerRoom {
    /// Unsubscribes [`InnerRoom`] from all its subscriptions.
    fn drop(&mut self) {
        self.rpc.unsub();

        if let CloseReason::ByClient { reason, .. } =
            *self.close_reason.borrow()
        {
            self.rpc.set_close_reason(reason);
        };

        self.on_close
            .call(RoomCloseReason::new(*self.close_reason.borrow()))
            .map(|result| result.map_err(console_error));
    }
}
