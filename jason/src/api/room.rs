//! Medea room.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::Deref as _,
    rc::{Rc, Weak},
};

use async_trait::async_trait;
use derive_more::Display;
use futures::{channel::mpsc, future, StreamExt as _};
use js_sys::Promise;
use medea_client_api_proto::{
    Command, ConnectionQualityScore, Direction, Event as RpcEvent,
    EventHandler, IceCandidate, IceConnectionState, IceServer, MediaSourceKind,
    MemberId, NegotiationRole, PeerConnectionState, PeerId, PeerMetrics, Track,
    TrackId, TrackUpdate,
};
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, spawn_local};

use crate::{
    api::connection::Connections,
    media::{
        LocalTracksConstraints, MediaKind, MediaStreamSettings,
        MediaStreamTrack, RecvConstraints,
    },
    peer::{
        MediaConnectionsError, MediaState, PeerConnection, PeerError,
        PeerEvent, PeerEventHandler, PeerRepository, RtcStats,
        StableMediaExchangeState, StableMuteState, TrackDirection,
    },
    rpc::{
        ClientDisconnect, CloseReason, ReconnectHandle, RpcClient,
        RpcClientError, TransportError,
    },
    utils::{Callback1, HandlerDetachedError, JasonError, JsCaused, JsError},
    JsMediaSourceKind,
};

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

    /// Returned if the previously added local media tracks does not satisfy
    /// the tracks sent from the media server.
    #[display(fmt = "Invalid local tracks: {}", _0)]
    InvalidLocalTracks(#[js(cause)] PeerError),

    /// Returned if [`PeerConnection`] cannot receive the local tracks from
    /// [`MediaManager`].
    #[display(fmt = "Failed to get local tracks: {}", _0)]
    CouldNotGetLocalMedia(#[js(cause)] PeerError),

    /// Returned if the requested [`PeerConnection`] is not found.
    #[display(fmt = "Peer with id {} doesnt exist", _0)]
    NoSuchPeer(PeerId),

    /// Returned if an error occurred during the webrtc signaling process
    /// with remote peer.
    #[display(fmt = "Some PeerConnection error: {}", _0)]
    PeerConnectionError(#[js(cause)] PeerError),

    /// Returned if was received event [`PeerEvent::NewRemoteTracks`] without
    /// [`Connection`] with remote [`Member`].
    #[display(fmt = "Remote stream from unknown member")]
    UnknownRemoteMember,

    /// Returned if [`MediaStreamTrack`] update failed.
    #[display(fmt = "Failed to update Track with {} ID.", _0)]
    FailedTrackPatch(TrackId),

    /// Typically, returned if [`RoomHandle::disable_audio`]-like functions
    /// called simultaneously.
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
            MediaConnections, MediaManager, RtcPeerConnection, TracksRequest,
        };

        match err {
            MediaConnections(ref e) => match e {
                MediaConnectionsError::InvalidTrackPatch(id) => {
                    Self::FailedTrackPatch(*id)
                }
                _ => Self::InvalidLocalTracks(err),
            },
            TracksRequest(_) => Self::InvalidLocalTracks(err),
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
#[wasm_bindgen]
pub struct RoomHandle(Weak<InnerRoom>);

impl RoomHandle {
    /// Implements externally visible `RoomHandle::join`.
    ///
    /// # Errors
    ///
    /// With [`RoomError::CallbackNotSet`] if `on_failed_local_media` or
    /// `on_connection_loss` callbacks are not set.
    ///
    /// With [`RoomError::CouldNotConnectToServer`] if cannot connect to media
    /// server.
    pub async fn inner_join(&self, token: String) -> Result<(), JasonError> {
        let inner = upgrade_or_detached!(self.0, JasonError)?;

        if !inner.on_failed_local_media.is_set() {
            return Err(JasonError::from(tracerr::new!(
                RoomError::CallbackNotSet("Room.on_failed_local_media()")
            )));
        }

        if !inner.on_connection_loss.is_set() {
            return Err(JasonError::from(tracerr::new!(
                RoomError::CallbackNotSet("Room.on_connection_loss()")
            )));
        }

        Rc::clone(&inner.rpc)
            .connect(token)
            .await
            .map_err(tracerr::map_from_and_wrap!( => RoomError))?;

        let mut connection_loss_stream = inner.rpc.on_connection_loss();
        let weak_inner = Rc::downgrade(&inner);
        spawn_local(async move {
            while connection_loss_stream.next().await.is_some() {
                if let Some(inner) = weak_inner.upgrade() {
                    let reconnect_handle =
                        ReconnectHandle::new(Rc::downgrade(&inner.rpc));
                    inner.on_connection_loss.call(reconnect_handle);
                } else {
                    log::error!("Inner Room dropped unexpectedly");
                    break;
                }
            }
        });

        Ok(())
    }

    /// Enables or disables specified media and source types publish or receival
    /// in all [`PeerConnection`]s.
    async fn set_track_media_state(
        &self,
        new_state: MediaState,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), JasonError> {
        let inner = upgrade_or_detached!(self.0, JasonError)?;
        inner.set_constraints_media_state(
            new_state,
            kind,
            direction,
            source_kind,
        );
        while !inner.is_all_peers_in_media_state(
            kind,
            direction,
            source_kind,
            new_state,
        ) {
            inner
                .toggle_media_state(new_state, kind, direction, source_kind)
                .await
                .map_err::<Traced<RoomError>, _>(|e| {
                    inner.set_constraints_media_state(
                        new_state.opposite(),
                        kind,
                        direction,
                        source_kind,
                    );
                    tracerr::new!(e)
                })?;
        }
        Ok(())
    }

    /// Returns [`Promise`] which will switch [`MediaState`] of the provided
    /// [`MediaKind`], [`TrackDirection`] and [`JsMediaSourceKind`] to the
    /// provided [`MediaState`].
    fn media_state_toggle_promise<S>(
        &self,
        media_state: S,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<JsMediaSourceKind>,
    ) -> Promise
    where
        S: Into<MediaState> + 'static,
    {
        let this = Self(self.0.clone());
        future_to_promise(async move {
            this.set_track_media_state(
                media_state.into(),
                kind,
                direction,
                source_kind.map(Into::into),
            )
            .await?;
            Ok(JsValue::UNDEFINED)
        })
    }
}

#[wasm_bindgen]
impl RoomHandle {
    /// Sets callback, which will be invoked when new [`Connection`] with some
    /// remote `Peer` is established.
    pub fn on_new_connection(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.connections.on_new_connection(f))
    }

    /// Sets `on_close` callback, which will be invoked on [`Room`] close,
    /// providing [`RoomCloseReason`].
    pub fn on_close(&mut self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0).map(|inner| inner.on_close.set_func(f))
    }

    /// Sets callback, which will be invoked when new local [`MediaStreamTrack`]
    /// will be added to this [`Room`].
    /// This might happen in such cases:
    /// 1. Media server initiates media request.
    /// 2. `disable_audio`/`enable_video` is called.
    /// 3. [`MediaStreamSettings`] updated via `set_local_media_settings`.
    pub fn on_local_track(&self, f: js_sys::Function) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.on_local_track.set_func(f))
    }

    /// Sets `on_failed_local_media` callback, which will be invoked on local
    /// media acquisition failures.
    pub fn on_failed_local_media(
        &self,
        f: js_sys::Function,
    ) -> Result<(), JsValue> {
        upgrade_or_detached!(self.0)
            .map(|inner| inner.on_failed_local_media.set_func(f))
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
    /// - `on_failed_local_media` callback is not set
    /// - `on_connection_loss` callback is not set
    /// - unable to connect to media server.
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
    /// Media obtaining/injection errors are fired to `on_failed_local_media`
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
            inner?
                .set_local_media_settings(settings)
                .await
                .map_err(JasonError::from)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Mutes outbound audio in this [`Room`].
    pub fn mute_audio(&self) -> Promise {
        self.media_state_toggle_promise(
            StableMuteState::Muted,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
    }

    /// Unmutes outbound audio in this [`Room`].
    pub fn unmute_audio(&self) -> Promise {
        self.media_state_toggle_promise(
            StableMuteState::Unmuted,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
    }

    /// Mutes outbound video in this [`Room`].
    pub fn mute_video(
        &self,
        source_kind: Option<JsMediaSourceKind>,
    ) -> Promise {
        self.media_state_toggle_promise(
            StableMuteState::Muted,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
    }

    /// Unmutes outbound video in this [`Room`].
    pub fn unmute_video(
        &self,
        source_kind: Option<JsMediaSourceKind>,
    ) -> Promise {
        self.media_state_toggle_promise(
            StableMuteState::Unmuted,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
    }

    /// Disables outbound audio in this [`Room`].
    pub fn disable_audio(&self) -> Promise {
        self.media_state_toggle_promise(
            StableMediaExchangeState::Disabled,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
    }

    /// Enables outbound audio in this [`Room`].
    pub fn enable_audio(&self) -> Promise {
        self.media_state_toggle_promise(
            StableMediaExchangeState::Enabled,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
    }

    /// Mutes outbound video.
    ///
    /// Affects only video with specific [`JsMediaSourceKind`] if specified.
    pub fn disable_video(
        &self,
        source_kind: Option<JsMediaSourceKind>,
    ) -> Promise {
        self.media_state_toggle_promise(
            StableMediaExchangeState::Disabled,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
    }

    /// Enables outbound video.
    ///
    /// Affects only video with specific [`JsMediaSourceKind`] if specified.
    pub fn enable_video(
        &self,
        source_kind: Option<JsMediaSourceKind>,
    ) -> Promise {
        self.media_state_toggle_promise(
            StableMediaExchangeState::Enabled,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
    }

    /// Disables inbound audio in this [`Room`].
    pub fn disable_remote_audio(&self) -> Promise {
        self.media_state_toggle_promise(
            StableMediaExchangeState::Disabled,
            MediaKind::Audio,
            TrackDirection::Recv,
            None,
        )
    }

    /// Disables inbound video in this [`Room`].
    pub fn disable_remote_video(&self) -> Promise {
        self.media_state_toggle_promise(
            StableMediaExchangeState::Disabled,
            MediaKind::Video,
            TrackDirection::Recv,
            None,
        )
    }

    /// Enables inbound audio in this [`Room`].
    pub fn enable_remote_audio(&self) -> Promise {
        self.media_state_toggle_promise(
            StableMediaExchangeState::Enabled,
            MediaKind::Audio,
            TrackDirection::Recv,
            None,
        )
    }

    /// Enables inbound video in this [`Room`].
    pub fn enable_remote_video(&self) -> Promise {
        self.media_state_toggle_promise(
            StableMediaExchangeState::Enabled,
            MediaKind::Video,
            TrackDirection::Recv,
            None,
        )
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

        let room = Rc::new(InnerRoom::new(rpc, peers, tx));
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
                        log::error!("Inner Room dropped unexpectedly");
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
                                        RoomError::InvalidLocalTracks(_)
                                        | RoomError::CouldNotGetLocalMedia(_) =>
                                        {
                                            let e =
                                                JasonError::from((err, trace));
                                            e.print();
                                            inner.on_failed_local_media.call(e);
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
    /// Used to inspect [`Room`]'s inner state in integration tests.
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

    /// Constraints to local [`MediaStream`] that is being published by
    /// [`PeerConnection`]s in this [`Room`].
    send_constraints: LocalTracksConstraints,

    /// Constraints to the [`MediaStream`]s received by [`PeerConnection`]s in
    /// this [`Room`]. Used to disable or enable media receiving.
    recv_constraints: Rc<RecvConstraints>,

    /// [`PeerConnection`] repository.
    peers: Box<dyn PeerRepository>,

    /// Channel for send events produced [`PeerConnection`] to [`Room`].
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Collection of [`Connection`]s with a remote [`Member`]s.
    connections: Connections,

    /// Callback to be invoked when new local [`MediaStreamTrack`] will be
    /// added to this [`Room`].
    on_local_track: Callback1<MediaStreamTrack>,

    /// Callback to be invoked when failed obtain [`MediaTrack`]s from
    /// [`MediaManager`] or failed inject stream into [`PeerConnection`].
    on_failed_local_media: Rc<Callback1<JasonError>>,

    /// Callback to be invoked when [`RpcClient`] loses connection.
    on_connection_loss: Callback1<ReconnectHandle>,

    /// JS callback which will be called when this [`Room`] will be closed.
    on_close: Rc<Callback1<RoomCloseReason>>,

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
    ) -> Self {
        Self {
            rpc,
            send_constraints: LocalTracksConstraints::default(),
            recv_constraints: Rc::new(RecvConstraints::default()),
            peers,
            peer_event_sender,
            connections: Connections::default(),
            on_connection_loss: Callback1::default(),
            on_failed_local_media: Rc::new(Callback1::default()),
            on_local_track: Callback1::default(),
            on_close: Rc::new(Callback1::default()),
            close_reason: RefCell::new(CloseReason::ByClient {
                reason: ClientDisconnect::RoomUnexpectedlyDropped,
                is_err: true,
            }),
        }
    }

    /// Toggles [`InnerRoom::recv_constraints`] or
    /// [`InnerRoom::send_constraints`] media exchange status based on the
    /// provided [`TrackDirection`], [`MediaKind`] and [`MediaSourceKind`].
    fn set_constraints_media_state(
        &self,
        state: MediaState,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
    ) {
        if let TrackDirection::Recv = direction {
            match state {
                MediaState::MediaExchange(media_exchange) => {
                    self.recv_constraints.set_enabled(
                        media_exchange == StableMediaExchangeState::Enabled,
                        kind,
                    );
                }
                MediaState::Mute(_) => {
                    unreachable!("Receivers muting is not implemented")
                }
            }
        } else {
            self.send_constraints
                .set_media_state(state, kind, source_kind);
        }
    }

    /// Sets `close_reason` of [`InnerRoom`].
    ///
    /// [`Drop`] implementation of [`InnerRoom`] is supposed
    /// to be triggered after this function call.
    fn set_close_reason(&self, reason: CloseReason) {
        self.close_reason.replace(reason);
    }

    /// Toggles [`TransceiverSide`]s [`MediaState`] by provided
    /// [`MediaKind`] in all [`PeerConnection`]s in this [`Room`].
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    #[allow(clippy::filter_map)]
    async fn toggle_media_state(
        &self,
        state: MediaState,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), Traced<RoomError>> {
        let disable_tracks: HashMap<_, _> = self
            .peers
            .get_all()
            .into_iter()
            .map(|peer| {
                let new_media_exchange_states = peer
                    .get_transceivers_sides(kind, direction, source_kind)
                    .into_iter()
                    .filter(|transceiver| transceiver.is_transitable())
                    .map(|transceiver| (transceiver.track_id(), state))
                    .collect();
                (peer.id(), new_media_exchange_states)
            })
            .collect();

        self.update_media_states(disable_tracks).await
    }

    /// Updates [`MediaState`]s of the [`TransceiverSide`] with a
    /// provided [`PeerId`] and [`TrackId`] to a provided
    /// [`MediaState`]s.
    #[allow(clippy::filter_map)]
    async fn update_media_states(
        &self,
        desired_media_states: HashMap<PeerId, HashMap<TrackId, MediaState>>,
    ) -> Result<(), Traced<RoomError>> {
        future::try_join_all(
            desired_media_states
                .into_iter()
                .filter_map(|(peer_id, desired_media_states)| {
                    self.peers
                        .get(peer_id)
                        .map(|peer| (peer, desired_media_states))
                })
                .map(|(peer, desired_media_states)| {
                    let peer_id = peer.id();
                    let mut transitions_futs = Vec::new();
                    let mut tracks_patches = Vec::new();
                    desired_media_states
                        .into_iter()
                        .filter_map(move |(track_id, desired_media_state)| {
                            peer.get_transceiver_side_by_id(track_id)
                                .map(|trnscvr| (trnscvr, desired_media_state))
                        })
                        .filter_map(|(trnscvr, desired_media_state)| {
                            if trnscvr
                                .is_subscription_needed(desired_media_state)
                            {
                                let is_patch_needed = trnscvr
                                    .is_track_patch_needed(desired_media_state);
                                Some((
                                    trnscvr,
                                    desired_media_state,
                                    is_patch_needed,
                                ))
                            } else {
                                None
                            }
                        })
                        .map(
                            |(
                                trnscvr,
                                desired_media_state,
                                should_be_patched,
                            )| {
                                trnscvr.media_state_transition_to(
                                    desired_media_state,
                                )?;
                                transitions_futs.push(
                                    trnscvr.when_media_state_stable(
                                        desired_media_state,
                                    ),
                                );
                                if should_be_patched {
                                    tracks_patches.push(
                                        desired_media_state
                                            .generate_track_patch(
                                                trnscvr.track_id(),
                                            ),
                                    );
                                }

                                Ok(())
                            },
                        )
                        .collect::<Result<(), _>>()
                        .map_err(tracerr::map_from_and_wrap!(=> RoomError))?;
                    if !tracks_patches.is_empty() {
                        self.rpc.send_command(Command::UpdateTracks {
                            peer_id,
                            tracks_patches,
                        });
                    }

                    Ok(future::try_join_all(transitions_futs))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(tracerr::map_from_and_wrap!())?,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())?;
        Ok(())
    }

    /// Returns `true` if all [`Sender`]s or [`Receiver`]s with a provided
    /// [`MediaKind`] and [`MediaSourceKind`] of this [`Room`] are in the
    /// provided `media_exchange_state`.
    pub fn is_all_peers_in_media_state(
        &self,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
        state: MediaState,
    ) -> bool {
        self.peers
            .get_all()
            .into_iter()
            .find(|p| {
                !p.is_all_transceiver_sides_in_media_state(
                    kind,
                    direction,
                    source_kind,
                    state,
                )
            })
            .is_none()
    }

    /// Updates this [`Room`]s [`MediaStreamSettings`]. This affects all
    /// [`PeerConnection`]s in this [`Room`]. If [`MediaStreamSettings`] is
    /// configured for some [`Room`], then this [`Room`] can only send
    /// [`MediaStream`] that corresponds to this settings.
    /// [`MediaStreamSettings`] update will change [`MediaStreamTrack`]s in all
    /// sending peers, so that might cause new [getUserMedia()][1] request.
    ///
    /// Media obtaining/injection errors are fired to `on_failed_local_media`
    /// callback.
    ///
    /// Will update [`MediaExchangeState`]s of the [`Sender`]s which are should
    /// be enabled or disabled.
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [1]: https://tinyurl.com/rnxcavf
    async fn set_local_media_settings(
        &self,
        settings: MediaStreamSettings,
    ) -> Result<(), Traced<RoomError>> {
        self.send_constraints.constrain(settings);

        let mut media_exchange_states_update = HashMap::new();
        for peer in self.peers.get_all() {
            match peer
                .update_local_stream()
                .await
                .map_err(tracerr::map_from_and_wrap!(=> RoomError))
            {
                Ok(new_media_exchange_states) => {
                    media_exchange_states_update.insert(
                        peer.id(),
                        new_media_exchange_states
                            .into_iter()
                            .map(|(id, s)| (id, s.into()))
                            .collect(),
                    );
                }
                Err(err) => {
                    self.on_failed_local_media.call(JasonError::from(err));
                }
            }
        }

        self.update_media_states(media_exchange_states_update)
            .await
            .map_err(tracerr::map_from_and_wrap!())
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
                self.rpc.send_command(Command::MakeSdpOffer {
                    peer_id: peer.id(),
                    sdp_offer,
                    transceivers_statuses: peer.get_transceivers_statuses(),
                    mids,
                });
            }
            Some(NegotiationRole::Answerer(offer)) => {
                let sdp_answer = peer
                    .process_offer(offer, tracks)
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;
                self.rpc.send_command(Command::MakeSdpAnswer {
                    peer_id: peer.id(),
                    sdp_answer,
                    transceivers_statuses: peer.get_transceivers_statuses(),
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
    ) -> Self::Output {
        let peer = self
            .peers
            .create_peer(
                peer_id,
                ice_servers,
                self.peer_event_sender.clone(),
                is_force_relayed,
                self.send_constraints.clone(),
                Rc::clone(&self.recv_constraints),
            )
            .map_err(tracerr::map_from_and_wrap!())?;

        for track in &tracks {
            match &track.direction {
                Direction::Recv { sender, .. } => {
                    self.connections.create_connection(peer_id, sender);
                }
                Direction::Send { receivers, .. } => {
                    for receiver in receivers {
                        self.connections.create_connection(peer_id, receiver);
                    }
                }
            }
        }
        self.create_tracks_and_maybe_negotiate(
            peer,
            tracks,
            Some(negotiation_role),
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Applies specified SDP Answer to a specified [`PeerConnection`].
    async fn on_sdp_answer_made(
        &self,
        peer_id: PeerId,
        sdp_answer: String,
    ) -> Self::Output {
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
    ) -> Self::Output {
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
    async fn on_peers_removed(&self, peer_ids: Vec<PeerId>) -> Self::Output {
        peer_ids.iter().for_each(|id| {
            self.connections.close_connection(*id);
            self.peers.remove(*id);
        });
        Ok(())
    }

    /// Creates new `Track`s, updates existing [`Sender`]s/[`Receiver`]s with
    /// [`TrackUpdate`]s.
    ///
    /// Will start (re)negotiation process if `Some` [`NegotiationRole`] is
    /// provided.
    async fn on_tracks_applied(
        &self,
        peer_id: PeerId,
        updates: Vec<TrackUpdate>,
        negotiation_role: Option<NegotiationRole>,
    ) -> Self::Output {
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
                TrackUpdate::IceRestart => {
                    peer.restart_ice();
                }
            }
        }
        peer.patch_tracks(patches)
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

    /// Updates [`Connection`]'s [`ConnectionQualityScore`] by calling
    /// [`Connection::update_quality_score`].
    async fn on_connection_quality_updated(
        &self,
        partner_member_id: MemberId,
        quality_score: ConnectionQualityScore,
    ) -> Self::Output {
        let conn = self
            .connections
            .get(&partner_member_id)
            .ok_or_else(|| tracerr::new!(RoomError::UnknownRemoteMember))?;

        conn.update_quality_score(quality_score);

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
    ) -> Self::Output {
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
        sender_id: MemberId,
        track: MediaStreamTrack,
    ) -> Self::Output {
        let conn = self
            .connections
            .get(&sender_id)
            .ok_or_else(|| tracerr::new!(RoomError::UnknownRemoteMember))?;
        conn.add_remote_track(track);

        Ok(())
    }

    /// Invokes `on_local_stream` [`Room`]'s callback.
    async fn on_new_local_track(
        &self,
        track: MediaStreamTrack,
    ) -> Self::Output {
        self.on_local_track.call(track);
        Ok(())
    }

    /// Handles [`PeerEvent::IceConnectionStateChanged`] event and sends new
    /// state to RPC server.
    async fn on_ice_connection_state_changed(
        &self,
        peer_id: PeerId,
        ice_connection_state: IceConnectionState,
    ) -> Self::Output {
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
    ) -> Self::Output {
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
    ) -> Self::Output {
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
    ) -> Self::Output {
        let peer = self
            .peers
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(RoomError::NoSuchPeer(peer_id)))?;
        if let Err(err) = peer
            .update_local_stream()
            .await
            .map_err(tracerr::map_from_and_wrap!(=> RoomError))
        {
            self.on_failed_local_media.call(JasonError::from(err));
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

        if let Some(Err(e)) = self
            .on_close
            .call(RoomCloseReason::new(*self.close_reason.borrow()))
        {
            log::error!("Failed to call Room::on_close callback: {:?}", e);
        }
    }
}
