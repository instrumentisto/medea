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
use medea_reactive::ObservableHashMap;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, spawn_local};

use crate::{
    api::connection::Connections,
    media::{
        track::{local, remote},
        LocalTracksConstraints, MediaKind, MediaStreamSettings,
        RecvConstraints,
    },
    peer::{
        media_exchange_state, mute_state, LocalStreamUpdateCriteria,
        MediaConnectionsError, MediaState, PeerComponent, PeerConnection,
        PeerError, PeerEvent, PeerEventHandler, PeerRepository, PeerState,
        ReceiverState, RtcStats, SenderState, TrackDirection,
    },
    rpc::{
        ClientDisconnect, CloseReason, ConnectionInfo,
        ConnectionInfoParseError, ReconnectHandle, RpcSession, SessionError,
    },
    utils::{
        Callback1, Component, HandlerDetachedError, JasonError, JsCaused,
        JsError,
    },
    JsMediaSourceKind,
};
use medea_client_api_proto::stats::TrackStats;
use medea_reactive::collections::ProgressableHashMap;

pub struct RoomCtx {
    pub rpc: Rc<dyn RpcSession>,
}

pub struct RoomState {
    peers: RefCell<ObservableHashMap<PeerId, Rc<PeerState>>>,
}

impl RoomState {
    pub fn new() -> Self {
        Self {
            peers: RefCell::new(ObservableHashMap::new()),
        }
    }
}

type RoomComponent = Component<RoomState, RefCell<Weak<InnerRoom>>, RoomCtx>;

impl RoomComponent {
    pub fn spawn(&self) {
        self.spawn_task(
            self.state().peers.borrow().on_insert(),
            Self::handle_insert_peer,
        );
    }

    async fn handle_insert_peer(
        ctx: RefCell<Weak<InnerRoom>>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<RoomState>,
        (peer_id, new_peer): (PeerId, Rc<PeerState>),
    ) {
        log::debug!("Peer inserted");
        let room = ctx.borrow().upgrade().unwrap();
        let peer = PeerConnection::new(
            peer_id,
            room.peer_event_sender.clone(),
            new_peer.ice_servers().clone(),
            room.peers.media_manager(),
            new_peer.force_relay(),
            room.send_constraints.clone(),
            room.recv_constraints.clone(),
        )
        .unwrap();

        let component =
            Component::new_component(new_peer, peer, global_ctx.clone());
        component.spawn();

        room.peers.insert_peer(peer_id, component);
    }
}

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
    /// Creates new [`RoomCloseReason`] with provided [`CloseReason`]
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
pub enum RoomError {
    /// Returned if the mandatory callback wasn't set.
    #[display(fmt = "`{}` callback isn't set.", _0)]
    CallbackNotSet(&'static str),

    /// Returned if the previously added local media tracks does not satisfy
    /// the tracks sent from the media server.
    #[display(fmt = "Invalid local tracks: {}", _0)]
    InvalidLocalTracks(#[js(cause)] PeerError),

    /// Returned if [`PeerConnection`] cannot receive the local tracks from
    /// [`MediaManager`].
    ///
    /// [`MediaManager`]: crate::media::MediaManager
    #[display(fmt = "Failed to get local tracks: {}", _0)]
    CouldNotGetLocalMedia(#[js(cause)] PeerError),

    /// Returned if the requested [`PeerConnection`] is not found.
    #[display(fmt = "Peer with id {} doesnt exist", _0)]
    NoSuchPeer(PeerId),

    /// Returned if an error occurred during the WebRTC signaling process
    /// with remote peer.
    #[display(fmt = "Some PeerConnection error: {}", _0)]
    PeerConnectionError(#[js(cause)] PeerError),

    /// Returned if was received event [`PeerEvent::NewRemoteTrack`] without
    /// connection with remote `Member`.
    #[display(fmt = "Remote stream from unknown member")]
    UnknownRemoteMember,

    /// Returned if [`track`] update failed.
    ///
    /// [`track`]: crate::media::track
    #[display(fmt = "Failed to update Track with {} ID.", _0)]
    FailedTrackPatch(TrackId),

    /// Typically, returned if [`RoomHandle::disable_audio`]-like functions
    /// called simultaneously.
    #[display(fmt = "Some MediaConnectionsError: {}", _0)]
    MediaConnections(#[js(cause)] MediaConnectionsError),

    /// [`RpcSession`] returned [`SessionError`].
    #[display(fmt = "WebSocketSession error occurred: {}", _0)]
    SessionError(#[js(cause)] SessionError),
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

impl From<SessionError> for RoomError {
    #[inline]
    fn from(e: SessionError) -> Self {
        Self::SessionError(e)
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
    /// With [`RoomError::SessionError`] if cannot connect to media server.
    pub async fn inner_join(&self, url: String) -> Result<(), JasonError> {
        let inner = upgrade_or_detached!(self.0, JasonError)?;

        let connection_info: ConnectionInfo = url.parse().map_err(
            tracerr::map_from_and_wrap!(=> ConnectionInfoParseError),
        )?;

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
            .connect(connection_info)
            .await
            .map_err(tracerr::map_from_and_wrap!( => RoomError))?;

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

        // Enabled senders may require new tracks to be inserted.
        if let (
            MediaState::MediaExchange(media_exchange_state::Stable::Enabled),
            TrackDirection::Send,
        ) = (new_state, direction)
        {
            for peer in inner.peers.get_all() {
                peer.update_local_stream(
                    LocalStreamUpdateCriteria::from_kinds(kind, source_kind),
                )
                .await
                .map_err(tracerr::map_from_and_wrap!(=> RoomError))?;
            }
        }

        Ok(())
    }
}

#[wasm_bindgen]
impl RoomHandle {
    /// Sets callback, which will be invoked when new [`Connection`] with some
    /// remote `Peer` is established.
    ///
    /// [`Connection`]: crate::api::Connection
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

    /// Sets callback, which will be invoked when new [`local::Track`] will be
    /// added to this [`Room`].
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

    /// Sets `on_connection_loss` callback, which will be invoked on connection
    /// with server loss.
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
    /// configured for some [`Room`], then this [`Room`] can only send media
    /// tracks that correspond to this settings. [`MediaStreamSettings`]
    /// update will change media tracks in all sending peers, so that might
    /// cause new [getUserMedia()][1] request.
    ///
    /// Media obtaining/injection errors are fired to `on_failed_local_media`
    /// callback.
    ///
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
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

    /// Returns [`Promise`] which will switch [`MediaState`] of the provided
    /// [`MediaKind`], [`TrackDirection`] and [`JsMediaSourceKind`] to the
    /// provided [`MediaState`].
    ///
    /// Helper function for all the exported mute/unmute/enable/disable
    /// audio/video send/receive methods.
    fn change_media_state<S>(
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

    /// Mutes outbound audio in this [`Room`].
    pub fn mute_audio(&self) -> Promise {
        self.change_media_state(
            mute_state::Stable::Muted,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
    }

    /// Unmutes outbound audio in this [`Room`].
    pub fn unmute_audio(&self) -> Promise {
        self.change_media_state(
            mute_state::Stable::Unmuted,
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
        self.change_media_state(
            mute_state::Stable::Muted,
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
        self.change_media_state(
            mute_state::Stable::Unmuted,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
    }

    /// Disables outbound audio in this [`Room`].
    pub fn disable_audio(&self) -> Promise {
        self.change_media_state(
            media_exchange_state::Stable::Disabled,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
    }

    /// Enables outbound audio in this [`Room`].
    pub fn enable_audio(&self) -> Promise {
        self.change_media_state(
            media_exchange_state::Stable::Enabled,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
    }

    /// Disables outbound video.
    ///
    /// Affects only video with specific [`JsMediaSourceKind`] if specified.
    pub fn disable_video(
        &self,
        source_kind: Option<JsMediaSourceKind>,
    ) -> Promise {
        self.change_media_state(
            media_exchange_state::Stable::Disabled,
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
        self.change_media_state(
            media_exchange_state::Stable::Enabled,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
    }

    /// Disables inbound audio in this [`Room`].
    pub fn disable_remote_audio(&self) -> Promise {
        self.change_media_state(
            media_exchange_state::Stable::Disabled,
            MediaKind::Audio,
            TrackDirection::Recv,
            None,
        )
    }

    /// Disables inbound video in this [`Room`].
    pub fn disable_remote_video(&self) -> Promise {
        self.change_media_state(
            media_exchange_state::Stable::Disabled,
            MediaKind::Video,
            TrackDirection::Recv,
            None,
        )
    }

    /// Enables inbound audio in this [`Room`].
    pub fn enable_remote_audio(&self) -> Promise {
        self.change_media_state(
            media_exchange_state::Stable::Enabled,
            MediaKind::Audio,
            TrackDirection::Recv,
            None,
        )
    }

    /// Enables inbound video in this [`Room`].
    pub fn enable_remote_video(&self) -> Promise {
        self.change_media_state(
            media_exchange_state::Stable::Enabled,
            MediaKind::Video,
            TrackDirection::Recv,
            None,
        )
    }
}

/// [`Weak`] reference upgradeable to the [`Room`].
#[derive(Clone)]
pub struct WeakRoom(Weak<InnerRoom>);

impl WeakRoom {
    /// Upgrades this [`WeakRoom`] to the [`Room`].
    ///
    /// Returns [`None`] if weak reference cannot be upgraded.
    #[inline]
    pub fn upgrade(&self) -> Option<Room> {
        self.0.upgrade().map(Room)
    }
}

/// [`Room`] where all the media happens (manages concrete [`PeerConnection`]s,
/// handles media server events, etc).
///
/// For using [`Room`] on JS side, consider the [`RoomHandle`].
///
/// [`PeerConnection`]: crate::peer::PeerConnection
pub struct Room(Rc<InnerRoom>);

impl Room {
    /// Creates new [`Room`] and associates it with the provided [`RpcSession`].
    #[allow(clippy::mut_mut)]
    pub fn new(
        rpc: Rc<dyn RpcSession>,
        peers: Box<dyn PeerRepository>,
    ) -> Self {
        enum RoomEvent {
            RpcEvent(RpcEvent),
            PeerEvent(PeerEvent),
            RpcClientLostConnection,
            RpcClientReconnected,
        }

        let (tx, peer_events_rx) = mpsc::unbounded();

        let mut rpc_events_stream =
            Rc::clone(&rpc).subscribe().map(RoomEvent::RpcEvent).fuse();
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

        let room = InnerRoom::new(rpc, peers, tx);
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

                if let Some(inner) = inner.upgrade() {
                    match event {
                        RoomEvent::RpcEvent(event) => {
                            if let Err(err) =
                                event.dispatch_with(inner.deref()).await
                            {
                                JasonError::from(err).print();
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
                    }
                } else {
                    log::error!("Inner Room dropped unexpectedly");
                    break;
                }
            }
        });

        Self(room)
    }

    pub fn insert_peer(&self, peer_id: PeerId, peer: PeerComponent) {
        self.0.peers.insert_peer(peer_id, peer)
    }

    /// Sets `close_reason` and consumes this [`Room`].
    ///
    /// [`Room`] [`Drop`] triggers `on_close` callback with provided
    /// [`CloseReason`].
    pub fn close(self, reason: CloseReason) {
        self.0.set_close_reason(reason);
    }

    /// Sets [`Room`]'s [`CloseReason`] to the provided value.
    #[inline]
    pub fn set_close_reason(&self, reason: CloseReason) {
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

    /// Indicates whether this [`Room`] reference is the same as the given
    /// [`Room`] reference. Compares pointers, not values.
    #[inline]
    pub fn ptr_eq(&self, other: &Room) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Checks [`RoomHandle`] equality by comparing inner pointers.
    #[inline]
    pub fn inner_ptr_eq(&self, handle: &RoomHandle) -> bool {
        handle
            .0
            .upgrade()
            .map_or(false, |handle_inner| Rc::ptr_eq(&self.0, &handle_inner))
    }

    /// Downgrades this [`Room`] to a [`WeakRoom`] reference.
    #[inline]
    pub fn downgrade(&self) -> WeakRoom {
        WeakRoom(Rc::downgrade(&self.0))
    }
}

/// Actual data of a [`Room`].
///
/// Shared between JS side ([`RoomHandle`]) and Rust side ([`Room`]).
struct InnerRoom {
    /// Client to talk with media server via Client API RPC.
    rpc: Rc<dyn RpcSession>,

    /// Constraints to local [`local::Track`]s that are being published by
    /// [`PeerConnection`]s in this [`Room`].
    send_constraints: LocalTracksConstraints,

    /// Constraints to the [`remote::Track`] received by [`PeerConnection`]s
    /// in this [`Room`]. Used to disable or enable media receiving.
    recv_constraints: Rc<RecvConstraints>,

    /// [`PeerConnection`] repository.
    peers: Box<dyn PeerRepository>,

    /// Channel for send events produced [`PeerConnection`] to [`Room`].
    peer_event_sender: mpsc::UnboundedSender<PeerEvent>,

    state: Rc<RoomState>,

    component: RoomComponent,

    /// Collection of [`Connection`]s with a remote `Member`s.
    ///
    /// [`Connection`]: crate::api::Connection
    connections: Connections,

    /// Callback to be invoked when new local [`local::JsTrack`] will be added
    /// to this [`Room`].
    on_local_track: Callback1<local::JsTrack>,

    /// Callback to be invoked when failed obtain [`local::Track`]s from
    /// [`MediaManager`] or failed inject stream into [`PeerConnection`].
    ///
    /// [`MediaManager`]: crate::media::MediaManager
    on_failed_local_media: Rc<Callback1<JasonError>>,

    /// Callback to be invoked when [`RpcSession`] loses connection.
    on_connection_loss: Callback1<ReconnectHandle>,

    /// JS callback which will be called when this [`Room`] will be closed.
    on_close: Rc<Callback1<RoomCloseReason>>,

    /// Reason of [`Room`] closing.
    ///
    /// This [`CloseReason`] will be provided into `on_close` JS callback.
    ///
    /// Note that `None` will be considered as error and `is_err` will be
    /// `true` in [`CloseReason`] provided to JS callback.
    close_reason: RefCell<CloseReason>,
}

impl InnerRoom {
    /// Creates new [`InnerRoom`].
    #[inline]
    fn new(
        rpc: Rc<dyn RpcSession>,
        peers: Box<dyn PeerRepository>,
        peer_event_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Rc<Self> {
        let room_state = Rc::new(RoomState::new());
        let room_component = RoomComponent::without_context(
            Rc::clone(&room_state),
            Rc::new(RoomCtx { rpc: rpc.clone() }),
        );
        let this = Rc::new(Self {
            rpc,
            send_constraints: LocalTracksConstraints::default(),
            recv_constraints: Rc::new(RecvConstraints::default()),
            peers,
            peer_event_sender,
            state: room_state,
            component: room_component,
            connections: Connections::default(),
            on_connection_loss: Callback1::default(),
            on_failed_local_media: Rc::new(Callback1::default()),
            on_local_track: Callback1::default(),
            on_close: Rc::new(Callback1::default()),
            close_reason: RefCell::new(CloseReason::ByClient {
                reason: ClientDisconnect::RoomUnexpectedlyDropped,
                is_err: true,
            }),
        });
        this.component.replace_context(Rc::downgrade(&this));
        this.component.spawn();

        this
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
        use media_exchange_state::Stable::Enabled;
        use MediaState::{MediaExchange, Mute};
        use TrackDirection::{Recv, Send};

        match (direction, state) {
            (Send, _) => {
                self.send_constraints
                    .set_media_state(state, kind, source_kind);
            }
            (Recv, MediaExchange(exchange)) => {
                self.recv_constraints.set_enabled(exchange == Enabled, kind);
            }
            (Recv, Mute(_)) => {
                unreachable!("Receivers muting is not implemented")
            }
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
    /// [`TransceiverSide`]: crate::peer::TransceiverSide
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
    ///
    /// [`TransceiverSide`]: crate::peer::TransceiverSide
    #[allow(clippy::filter_map)]
    async fn update_media_states(
        &self,
        desired_states: HashMap<PeerId, HashMap<TrackId, MediaState>>,
    ) -> Result<(), Traced<RoomError>> {
        future::try_join_all(
            desired_states
                .into_iter()
                .filter_map(|(peer_id, desired_states)| {
                    self.peers.get(peer_id).map(|peer| (peer, desired_states))
                })
                .map(|(peer, desired_states)| {
                    let peer_id = peer.id();
                    let mut transitions_futs = Vec::new();
                    let mut tracks_patches = Vec::new();
                    desired_states
                        .into_iter()
                        .filter_map(move |(track_id, desired_state)| {
                            peer.get_transceiver_side_by_id(track_id)
                                .map(|trnscvr| (trnscvr, desired_state))
                        })
                        .filter_map(|(trnscvr, desired_state)| {
                            if trnscvr.is_subscription_needed(desired_state) {
                                let need_patch = trnscvr
                                    .is_track_patch_needed(desired_state);
                                Some((trnscvr, desired_state, need_patch))
                            } else {
                                None
                            }
                        })
                        .map(|(trnscvr, desired_state, need_patch)| {
                            trnscvr.media_state_transition_to(desired_state)?;
                            transitions_futs.push(
                                trnscvr.when_media_state_stable(desired_state),
                            );
                            if need_patch {
                                tracks_patches.push(
                                    desired_state.generate_track_patch(
                                        trnscvr.track_id(),
                                    ),
                                );
                            }

                            Ok(())
                        })
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
    /// provided [`MediaState`].
    ///
    /// [`Sender`]: crate::peer::Sender
    /// [`Receiver`]: crate::peer::Receiver
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
    /// [`local::Track`]s that corresponds to this settings.
    /// [`MediaStreamSettings`] update will change [`local::Track`]s in all
    /// sending peers, so that might cause new [getUserMedia()][1] request.
    ///
    /// Media obtaining/injection errors are fired to `on_failed_local_media`
    /// callback.
    ///
    /// Will update [`media_exchange_state::Stable`]s of the [`Sender`]s which
    /// are should be enabled or disabled.
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    /// [`PeerConnection`]: crate::peer::PeerConnection
    /// [`Sender`]: crate::peer::Sender
    async fn set_local_media_settings(
        &self,
        settings: MediaStreamSettings,
    ) -> Result<(), Traced<RoomError>> {
        self.send_constraints.constrain(settings);

        let mut states_update = HashMap::new();
        for peer in self.peers.get_all() {
            peer.update_local_stream(LocalStreamUpdateCriteria::all())
                .await
                .map_err(tracerr::map_from_and_wrap!(=> RoomError))
                .map(|new_media_exchange_states| {
                    states_update.insert(
                        peer.id(),
                        new_media_exchange_states
                            .into_iter()
                            .map(|(id, s)| (id, s.into()))
                            .collect(),
                    );
                })?;
        }

        self.update_media_states(states_update)
            .await
            .map_err(tracerr::map_from_and_wrap!())
    }

    /// Stops state transition timers in all [`PeerConnection`]'s in this
    /// [`Room`].
    fn handle_rpc_connection_lost(&self) {
        for peer in self.peers.get_all() {
            peer.stop_state_transitions_timers();
        }
        self.on_connection_loss
            .call(ReconnectHandle::new(Rc::downgrade(&self.rpc)));
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
    ///
    /// [`Receiver`]: crate::peer::Receiver
    /// [`Sender`]: crate::peer::Sender
    async fn create_tracks_and_maybe_negotiate(
        &self,
        peer: Rc<PeerConnection>,
        tracks: Vec<Track>,
        negotiation_role: Option<NegotiationRole>,
        maybe_update_local_media: bool,
    ) -> Result<(), Traced<RoomError>> {
        match negotiation_role {
            None => {
                peer.create_tracks(tracks)
                    .map_err(tracerr::map_from_and_wrap!())?;
            }
            Some(NegotiationRole::Offerer) => {
                let sdp_offer = peer
                    .get_offer(tracks, maybe_update_local_media)
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
                    .process_offer(offer, tracks, maybe_update_local_media)
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
    ///
    /// [`Connection`]: crate::api::Connection
    async fn on_peer_created(
        &self,
        peer_id: PeerId,
        negotiation_role: NegotiationRole,
        tracks: Vec<Track>,
        ice_servers: Vec<IceServer>,
        is_force_relayed: bool,
    ) -> Self::Output {
        // let peer = self
        //     .peers
        //     .create_peer(
        //         peer_id,
        //         ice_servers,
        //         self.peer_event_sender.clone(),
        //         is_force_relayed,
        //         self.send_constraints.clone(),
        //         Rc::clone(&self.recv_constraints),
        //     )
        //     .map_err(tracerr::map_from_and_wrap!())?;

        let mut senders = ProgressableHashMap::new();
        let mut receivers = ProgressableHashMap::new();
        for track in &tracks {
            match &track.direction {
                Direction::Send { receivers, mid } => {
                    senders.insert(
                        track.id,
                        Rc::new(SenderState::new(
                            track.id,
                            mid.clone(),
                            track.media_type.clone(),
                            receivers.clone(),
                        )),
                    );
                }
                Direction::Recv { sender, mid } => {
                    receivers.insert(
                        track.id,
                        Rc::new(ReceiverState::new(
                            track.id,
                            mid.clone(),
                            track.media_type.clone(),
                            sender.clone(),
                        )),
                    );
                }
            }
        }
        let peer_state = PeerState::new(
            senders,
            receivers,
            peer_id,
            ice_servers,
            is_force_relayed,
            Some(negotiation_role),
        );
        self.state
            .peers
            .borrow_mut()
            .insert(peer_id, Rc::new(peer_state));

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

        Ok(())

        // self.create_tracks_and_maybe_negotiate(
        //     peer,
        //     tracks,
        //     Some(negotiation_role),
        //     true,
        // )
        // .await
        // .map_err(tracerr::map_from_and_wrap!())
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
    ///
    /// [`Receiver`]: crate::peer::Receiver
    /// [`Sender`]: crate::peer::Sender
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
        // let mut new_tracks = Vec::new();
        // let mut patches = Vec::new();
        let peer_state =
            self.state.peers.borrow().get(&peer_id).unwrap().clone();

        for update in updates {
            match update {
                TrackUpdate::Added(track) => match track.direction {
                    Direction::Send { receivers, mid } => {
                        let sender_state = SenderState::new(
                            track.id,
                            mid,
                            track.media_type,
                            receivers,
                        );
                        peer_state
                            .insert_sender(track.id, Rc::new(sender_state));
                    }
                    Direction::Recv { sender, mid } => {
                        let receiver_state = ReceiverState::new(
                            track.id,
                            mid,
                            track.media_type,
                            sender,
                        );
                        peer_state
                            .insert_receiver(track.id, Rc::new(receiver_state));
                    }
                },
                TrackUpdate::Updated(track_patch) => {
                    if let Some(sender) = peer_state.get_sender(track_patch.id)
                    {
                        sender.update(track_patch);
                    } else if let Some(receiver) =
                        peer_state.get_receiver(track_patch.id)
                    {
                        receiver.update(track_patch);
                    }
                }
                TrackUpdate::IceRestart => {
                    peer.restart_ice();
                }
            }
        }
        if let Some(negotiation_role) = negotiation_role {
            peer_state.set_negotiation_role(negotiation_role);
        }

        // let kinds = peer
        //     .patch_tracks(patches)
        //     .await
        //     .map_err(tracerr::map_from_and_wrap!())?;
        // peer.update_local_stream(kinds)
        //     .await
        //     .map_err(tracerr::map_from_and_wrap!())?;
        // self.create_tracks_and_maybe_negotiate(
        //     peer,
        //     new_tracks,
        //     negotiation_role,
        //     false,
        // )
        // .await
        // .map_err(tracerr::map_from_and_wrap!())?;
        Ok(())
    }

    /// Updates [`Connection`]'s [`ConnectionQualityScore`] by calling
    /// [`Connection::update_quality_score()`][1].
    ///
    /// [`Connection`]: crate::api::Connection
    /// [1]: crate::api::Connection::update_quality_score
    async fn on_connection_quality_updated(
        &self,
        partner_member_id: MemberId,
        quality_score: ConnectionQualityScore,
    ) -> Self::Output {
        if let Some(conn) = self.connections.get(&partner_member_id) {
            conn.update_quality_score(quality_score);
        }
        Ok(())
    }

    #[inline]
    async fn on_room_joined(&self, _: MemberId) -> Self::Output {
        unreachable!("Room can't receive Event::RoomJoined")
    }

    #[inline]
    async fn on_room_left(
        &self,
        _: medea_client_api_proto::CloseReason,
    ) -> Self::Output {
        unreachable!("Room can't receive Event::RoomLeft")
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
    /// [`remote::Track`] to the related [`Connection`].
    ///
    /// [`Connection`]: crate::api::Connection
    /// [`Stream`]: futures::Stream
    async fn on_new_remote_track(
        &self,
        sender_id: MemberId,
        track: remote::Track,
    ) -> Self::Output {
        let conn = self
            .connections
            .get(&sender_id)
            .ok_or_else(|| tracerr::new!(RoomError::UnknownRemoteMember))?;
        conn.add_remote_track(track);

        Ok(())
    }

    /// Invokes `on_local_track` [`Room`]'s callback.
    async fn on_new_local_track(
        &self,
        track: Rc<local::Track>,
    ) -> Self::Output {
        self.on_local_track.call(local::JsTrack::new(track));
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

    /// Handles [`PeerEvent::FailedLocalMedia`] event by invoking
    /// `on_failed_local_media` [`Room`]'s callback.
    async fn on_failed_local_media(&self, error: JasonError) -> Self::Output {
        self.on_failed_local_media.call(error);
        Ok(())
    }
}

impl Drop for InnerRoom {
    /// Unsubscribes [`InnerRoom`] from all its subscriptions.
    fn drop(&mut self) {
        if let CloseReason::ByClient { reason, .. } =
            *self.close_reason.borrow()
        {
            self.rpc.close_with_reason(reason);
        };

        if let Some(Err(e)) = self
            .on_close
            .call(RoomCloseReason::new(*self.close_reason.borrow()))
        {
            log::error!("Failed to call Room::on_close callback: {:?}", e);
        }
    }
}
