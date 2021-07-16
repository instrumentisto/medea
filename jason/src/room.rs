//! Medea [`Room`].

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
};

use async_recursion::async_recursion;
use async_trait::async_trait;
use derive_more::{Display, From};
use futures::{channel::mpsc, future, FutureExt as _, StreamExt as _};
use medea_client_api_proto::{
    self as proto, Command, ConnectionQualityScore, Event as RpcEvent,
    EventHandler, IceCandidate, IceConnectionState, IceServer, MemberId,
    NegotiationRole, PeerConnectionState, PeerId, PeerMetrics, PeerUpdate,
    Track, TrackId,
};
use tracerr::Traced;

use crate::{
    api,
    connection::Connections,
    media::{
        track::{local, remote},
        InitLocalTracksError, LocalTracksConstraints, MediaKind, MediaManager,
        MediaSourceKind, MediaStreamSettings, RecvConstraints,
    },
    peer::{
        self, media::ProhibitedStateError, media_exchange_state, mute_state,
        InsertLocalTracksError, LocalMediaError, LocalStreamUpdateCriteria,
        MediaState, PeerConnection, PeerEvent, PeerEventHandler,
        TrackDirection, TracksRequestError, UpdateLocalStreamError,
    },
    platform,
    rpc::{
        ClientDisconnect, CloseReason, ConnectionInfo,
        ConnectionInfoParseError, ReconnectHandle, RpcSession, SessionError,
    },
    utils::{AsProtoState, JsCaused},
};

/// Reason of why [`Room`] has been closed.
///
/// This struct is passed into [`RoomHandle::on_close`] callback.
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
    /// Creates a new [`RoomCloseReason`] with the provided [`CloseReason`].
    ///
    /// `is_err` may be `true` only on closing by client.
    ///
    /// `is_closed_by_server` is `true` on [`CloseReason::ByServer`].
    #[must_use]
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

    /// Returns a close reason of the [`Room`].
    #[inline]
    #[must_use]
    pub fn reason(&self) -> String {
        self.reason.clone()
    }

    /// Indicates whether the [`Room`] was closed by server.
    #[inline]
    #[must_use]
    pub fn is_closed_by_server(&self) -> bool {
        self.is_closed_by_server
    }

    /// Indicates whether the [`Room`]'s close reason is considered as an error.
    #[inline]
    #[must_use]
    pub fn is_err(&self) -> bool {
        self.is_err
    }
}

/// Errors occurring in [`RoomHandle::join()`] method.
#[derive(Clone, Debug, Display, From, JsCaused)]
#[js(error = "platform::Error")]
pub enum RoomJoinError {
    /// [`RoomHandle`]'s [`Weak`] pointer is detached.
    #[display(fmt = "Room is in detached state")]
    Detached,

    /// Returned if the mandatory callback wasn't set.
    #[display(fmt = "`{}` callback isn't set.", _0)]
    #[from(ignore)]
    CallbackNotSet(&'static str),

    /// [`ConnectionInfo`] parsing failed.
    #[display(fmt = "Failed to parse ConnectionInfo: {}", _0)]
    ConnectionInfoParse(ConnectionInfoParseError),

    /// [`RpcSession`] returned [`SessionError`].
    #[display(fmt = "WebSocketSession error occurred: {}", _0)]
    SessionError(#[js(cause)] SessionError),
}

/// Error of [`RoomHandle`]'s [`Weak`] pointer being detached.
#[derive(Clone, Debug, Display, Eq, From, JsCaused, PartialEq)]
#[js(error = "platform::Error")]
pub struct HandleDetachedError;

/// Errors occurring when changing media state of [`Sender`]s and [`Receiver`]s.
///
/// [`Sender`]: peer::media::Sender
/// [`Receiver`]: peer::media::Receiver
#[derive(Clone, Debug, Display, From, JsCaused)]
#[js(error = "platform::Error")]
pub enum ChangeMediaStateError {
    /// [`RoomHandle`]'s [`Weak`] pointer is detached.
    #[display(fmt = "Room is in detached state")]
    Detached,

    /// Validating [`TracksRequest`] doesn't pass.
    ///
    /// [`TracksRequest`]: peer::TracksRequest
    InvalidLocalTracks(TracksRequestError),

    /// [`MediaManager`] failed to acquire [`local::Track`]s.
    CouldNotGetLocalMedia(#[js(cause)] InitLocalTracksError),

    /// [`local::Track`]s cannot be inserted into [`Sender`]s of some
    /// [`PeerConnection`] in this [`Room`].
    ///
    /// [`Sender`]: peer::media::Sender
    InsertLocalTracksError(#[js(cause)] InsertLocalTracksError),

    /// Requested state transition is not allowed by [`Sender`]'s settings.
    ///
    /// [`Sender`]: peer::media::Sender
    ProhibitedState(ProhibitedStateError),

    /// [`MediaState`] of a [`Sender`] transits to an opposite of the requested
    /// one.
    ///
    /// [`Sender`]: peer::media::Sender
    #[display(
        fmt = "MediaState of Sender transits to opposite ({}) of the \
               requested MediaExchangeState",
        _0
    )]
    TransitionIntoOppositeState(MediaState),
}

impl From<GetLocalTracksError> for ChangeMediaStateError {
    #[inline]
    fn from(err: GetLocalTracksError) -> Self {
        match err {
            GetLocalTracksError::InvalidLocalTracks(e) => Self::from(e),
            GetLocalTracksError::CouldNotGetLocalMedia(e) => Self::from(e),
        }
    }
}

impl From<UpdateLocalStreamError> for ChangeMediaStateError {
    #[inline]
    fn from(err: UpdateLocalStreamError) -> Self {
        use UpdateLocalStreamError as UpdateErr;
        match err {
            UpdateErr::InvalidLocalTracks(e) => Self::from(e),
            UpdateErr::CouldNotGetLocalMedia(e) => Self::from(e),
            UpdateErr::InsertLocalTracksError(e) => Self::from(e),
        }
    }
}

/// Errors occurring when a [`Room`] tries to acquire [`local::Track`]s via
/// [`MediaManager`].
#[derive(Clone, Debug, Display, From, JsCaused)]
#[js(error = "platform::Error")]
pub enum GetLocalTracksError {
    /// Validating [`TracksRequest`] doesn't pass.
    ///
    /// [`TracksRequest`]: peer::TracksRequest
    InvalidLocalTracks(TracksRequestError),

    /// [`MediaManager`] failed to acquire [`local::Track`]s.
    CouldNotGetLocalMedia(#[js(cause)] InitLocalTracksError),
}

macro_rules! upgrade_inner {
    ($v:expr) => {
        $v.upgrade()
            .ok_or_else(|| tracerr::new!(HandleDetachedError))
    };
}

/// External handle to a [`Room`].
#[derive(Clone)]
pub struct RoomHandle(Weak<InnerRoom>);

impl RoomHandle {
    /// Connects to a media server and joins the [`Room`] with the provided
    /// authorization `token`.
    ///
    /// Authorization token has a fixed format:
    /// `{{ Host URL }}/{{ Room ID }}/{{ Member ID }}?token={{ Auth Token }}`
    /// (e.g. `wss://medea.com/MyConf1/Alice?token=777`).
    ///
    /// Establishes connection with a media server (if it doesn't exist
    /// already ).
    ///
    /// # Errors
    ///
    /// See [`RoomJoinError`] for details.
    pub async fn join(&self, url: String) -> Result<(), Traced<RoomJoinError>> {
        let inner = self
            .0
            .upgrade()
            .ok_or_else(|| tracerr::new!(RoomJoinError::Detached))?;

        let connection_info: ConnectionInfo =
            url.parse().map_err(tracerr::map_from_and_wrap!())?;

        if !inner.on_failed_local_media.is_set() {
            return Err(tracerr::new!(RoomJoinError::CallbackNotSet(
                "Room.on_failed_local_media()"
            )));
        }

        if !inner.on_connection_loss.is_set() {
            return Err(tracerr::new!(RoomJoinError::CallbackNotSet(
                "Room.on_connection_loss()"
            )));
        }

        Rc::clone(&inner.rpc)
            .connect(connection_info)
            .await
            .map_err(tracerr::map_from_and_wrap!( => RoomJoinError))?;

        Ok(())
    }

    /// Sets callback, invoked when a new [`Connection`] with some remote `Peer`
    /// is established.
    ///
    /// # Errors
    ///
    /// See [`HandleDetachedError`] for details.
    ///
    /// [`Connection`]: crate::connection::Connection
    pub fn on_new_connection(
        &self,
        f: platform::Function<api::ConnectionHandle>,
    ) -> Result<(), Traced<HandleDetachedError>> {
        upgrade_inner!(self.0)
            .map(|inner| inner.connections.on_new_connection(f))
    }

    /// Sets `on_close` callback, invoked on this [`Room`] close, providing a
    /// [`RoomCloseReason`].
    ///
    /// # Errors
    ///
    /// See [`HandleDetachedError`] for details.
    pub fn on_close(
        &self,
        f: platform::Function<api::RoomCloseReason>,
    ) -> Result<(), Traced<HandleDetachedError>> {
        upgrade_inner!(self.0).map(|inner| inner.on_close.set_func(f))
    }

    /// Sets callback, invoked when a new [`local::Track`] is added to this
    /// [`Room`].
    ///
    /// This might happen in such cases:
    /// 1. Media server initiates a media request.
    /// 2. `enable_audio`/`enable_video` is called.
    /// 3. [`MediaStreamSettings`] updated via `set_local_media_settings`.
    ///
    /// # Errors
    ///
    /// See [`HandleDetachedError`] for details.
    pub fn on_local_track(
        &self,
        f: platform::Function<api::LocalMediaTrack>,
    ) -> Result<(), Traced<HandleDetachedError>> {
        upgrade_inner!(self.0).map(|inner| inner.on_local_track.set_func(f))
    }

    /// Sets `on_failed_local_media` callback, invoked on a local media
    /// acquisition failures.
    ///
    /// # Errors
    ///
    /// See [`HandleDetachedError`] for details.
    pub fn on_failed_local_media(
        &self,
        f: platform::Function<api::Error>,
    ) -> Result<(), Traced<HandleDetachedError>> {
        upgrade_inner!(self.0)
            .map(|inner| inner.on_failed_local_media.set_func(f))
    }

    /// Sets `on_connection_loss` callback, invoked when a connection with
    /// server is lost.
    ///
    /// # Errors
    ///
    /// See [`HandleDetachedError`] for details.
    pub fn on_connection_loss(
        &self,
        f: platform::Function<api::ReconnectHandle>,
    ) -> Result<(), Traced<HandleDetachedError>> {
        upgrade_inner!(self.0).map(|inner| inner.on_connection_loss.set_func(f))
    }

    /// Updates this [`Room`]s [`MediaStreamSettings`]. This affects all
    /// [`PeerConnection`]s in this [`Room`]. If [`MediaStreamSettings`] is
    /// configured for some [`Room`], then this [`Room`] can only send media
    /// tracks that correspond to this settings. [`MediaStreamSettings`]
    /// update will change media tracks in all sending peers, so that might
    /// cause new [getUserMedia()][1] request.
    ///
    /// Media obtaining/injection errors are additionally fired to
    /// `on_failed_local_media` callback.
    ///
    /// If `stop_first` set to `true` then affected [`local::Track`]s will be
    /// dropped before new [`MediaStreamSettings`] is applied. This is usually
    /// required when changing video source device due to hardware limitations,
    /// e.g. having an active track sourced from device `A` may hinder
    /// [getUserMedia()][1] requests to device `B`.
    ///
    /// `rollback_on_fail` option configures [`MediaStreamSettings`] update
    /// request to automatically rollback to previous settings if new settings
    /// cannot be applied.
    ///
    /// If recovering from fail state isn't possible then affected media types
    /// will be disabled.
    ///
    /// # Errors
    ///
    /// With [`ConstraintsUpdateError::Errored`] if and error has occurred while
    /// applying the provided [`MediaStreamSettings`].
    ///
    /// With [`ConstraintsUpdateError::Recovered`] if [`MediaStreamSettings`]
    /// are rolled-back because an error had occurred.
    ///
    /// With [`ConstraintsUpdateError::RecoverFailed`] if
    /// [`MediaStreamSettings`] rollback failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    pub async fn set_local_media_settings(
        &self,
        settings: MediaStreamSettings,
        stop_first: bool,
        rollback_on_fail: bool,
    ) -> Result<(), ConstraintsUpdateError> {
        let inner = (self.0).upgrade().ok_or_else(|| {
            ConstraintsUpdateError::errored(tracerr::new!(
                ChangeMediaStateError::Detached
            ))
        })?;

        inner
            .set_local_media_settings(settings, stop_first, rollback_on_fail)
            .await
    }

    /// Changes [`MediaState`] of the provided [`MediaKind`], [`TrackDirection`]
    /// and [`MediaSourceKind`] to the provided [`MediaState`].
    ///
    /// Helper function for all the exported mute/unmute/enable/disable
    /// audio/video send/receive methods.
    async fn change_media_state<S>(
        &self,
        new_state: S,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), Traced<ChangeMediaStateError>>
    where
        S: Into<MediaState> + 'static,
    {
        let inner = (self.0)
            .upgrade()
            .ok_or_else(|| tracerr::new!(ChangeMediaStateError::Detached))?;

        let new_state = new_state.into();
        let source_kind = source_kind.map(Into::into);

        inner.set_constraints_media_state(
            new_state,
            kind,
            direction,
            source_kind,
        );

        let direction_send = matches!(direction, TrackDirection::Send);
        let enabling = matches!(
            new_state,
            MediaState::MediaExchange(media_exchange_state::Stable::Enabled)
        );

        // Perform `getUserMedia()`/`getDisplayMedia()` right away, so we can
        // fail fast without touching senders states and starting all required
        // messaging.
        // Hold tracks through all process, to ensure that they will be reused
        // without additional requests.
        let _tracks_handles = if direction_send && enabling {
            inner
                .get_local_tracks(kind, source_kind)
                .await
                .map_err(tracerr::map_from_and_wrap!())?
        } else {
            Vec::new()
        };

        while !inner.is_all_peers_in_media_state(
            kind,
            direction,
            source_kind,
            new_state,
        ) {
            if let Err(e) = inner
                .toggle_media_state(new_state, kind, direction, source_kind)
                .await
                .map_err(tracerr::map_from_and_wrap!())
            {
                if direction_send && enabling {
                    inner.set_constraints_media_state(
                        new_state.opposite(),
                        kind,
                        direction,
                        source_kind,
                    );
                    inner
                        .toggle_media_state(
                            new_state.opposite(),
                            kind,
                            direction,
                            source_kind,
                        )
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                }
                return Err(e);
            }
        }

        Ok(())
    }

    /// Mutes outbound audio in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::unmute_audio()`] was called while muting or a media server
    /// didn't approve this state transition.
    #[inline]
    pub async fn mute_audio(
        &self,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            mute_state::Stable::Muted,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Unmutes outbound audio in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::mute_audio()`] was called while muting or a media server
    /// didn't approve this state transition.
    #[inline]
    pub async fn unmute_audio(
        &self,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            mute_state::Stable::Unmuted,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Mutes outbound video in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::unmute_video()`] was called while muting or a media server
    /// didn't approve this state transition.
    #[inline]
    pub async fn mute_video(
        &self,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            mute_state::Stable::Muted,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Unmutes outbound video in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::mute_video()`] was called while muting or a media server
    /// didn't approve this state transition.
    #[inline]
    pub async fn unmute_video(
        &self,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            mute_state::Stable::Unmuted,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Disables outbound audio in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::ProhibitedState`] if audio track's sender
    /// is configured as `required`.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::enable_audio()`] was called while disabling or a media
    /// server didn't approve this state transition.
    #[inline]
    pub async fn disable_audio(
        &self,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            media_exchange_state::Stable::Disabled,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Enables outbound audio in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::disable_audio()`] was called while enabling or a media
    /// server didn't approve this state transition.
    ///
    /// With [`ChangeMediaStateError::CouldNotGetLocalMedia`] if media
    /// acquisition request failed.
    #[inline]
    pub async fn enable_audio(
        &self,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            media_exchange_state::Stable::Enabled,
            MediaKind::Audio,
            TrackDirection::Send,
            None,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Disables outbound video.
    ///
    /// Affects only video with specific [`MediaSourceKind`] if specified.
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::ProhibitedState`] video track's sender is
    /// configured as `required`.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::enable_video()`] was called while disabling or a media
    /// server didn't approve this state transition.
    #[inline]
    pub async fn disable_video(
        &self,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            media_exchange_state::Stable::Disabled,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Enables outbound video.
    ///
    /// Affects only video with specific [`MediaSourceKind`] if specified.
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::disable_video()`] was called while enabling or a media
    /// server didn't approve this state transition.
    ///
    /// With [`ChangeMediaStateError::CouldNotGetLocalMedia`] if media
    /// acquisition request failed.
    #[inline]
    pub async fn enable_video(
        &self,
        source_kind: Option<MediaSourceKind>,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            media_exchange_state::Stable::Enabled,
            MediaKind::Video,
            TrackDirection::Send,
            source_kind,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Disables inbound audio in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::enable_remote_audio()`] was called while disabling or a
    /// media server didn't approve this state transition.
    #[inline]
    pub async fn disable_remote_audio(
        &self,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            media_exchange_state::Stable::Disabled,
            MediaKind::Audio,
            TrackDirection::Recv,
            None,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Disables inbound video in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::enable_remote_video()`] was called while disabling or a
    /// media server didn't approve this state transition.
    #[inline]
    pub async fn disable_remote_video(
        &self,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            media_exchange_state::Stable::Disabled,
            MediaKind::Video,
            TrackDirection::Recv,
            None,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Enables inbound audio in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::disable_remote_audio()`] was called while enabling or a
    /// media server didn't approve this state transition.
    #[inline]
    pub async fn enable_remote_audio(
        &self,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            media_exchange_state::Stable::Enabled,
            MediaKind::Audio,
            TrackDirection::Recv,
            None,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
    }

    /// Enables inbound video in this [`Room`].
    ///
    /// # Errors
    ///
    /// With [`ChangeMediaStateError::Detached`] if an inner [`Weak`] pointer
    /// upgrade fails.
    ///
    /// With [`ChangeMediaStateError::TransitionIntoOppositeState`] if
    /// [`RoomHandle::disable_remote_video()`] was called while enabling or a
    /// media server didn't approve this state transition.
    #[inline]
    pub async fn enable_remote_video(
        &self,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        self.change_media_state(
            media_exchange_state::Stable::Enabled,
            MediaKind::Video,
            TrackDirection::Recv,
            None,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())
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
pub struct Room(Rc<InnerRoom>);

impl Room {
    /// Creates new [`Room`] and associates it with the provided [`RpcSession`].
    #[allow(clippy::mut_mut)]
    pub fn new(
        rpc: Rc<dyn RpcSession>,
        media_manager: Rc<MediaManager>,
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

        let room = Rc::new(InnerRoom::new(rpc, media_manager, tx));
        let inner = Rc::downgrade(&room);

        platform::spawn(async move {
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
                            if let Err(e) = event
                                .dispatch_with(&*inner)
                                .await
                                .map_err(tracerr::wrap!(=> UnknownPeerIdError))
                            {
                                log::error!("{}", e);
                            };
                        }
                        RoomEvent::PeerEvent(event) => {
                            if let Err(e) =
                                event.dispatch_with(&*inner).await.map_err(
                                    tracerr::wrap!(=> UnknownRemoteMemberError),
                                )
                            {
                                log::error!("{}", e);
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

    /// Creates a new external handle to [`Room`]. You can create them as many
    /// as you need.
    #[inline]
    #[must_use]
    pub fn new_handle(&self) -> RoomHandle {
        RoomHandle(Rc::downgrade(&self.0))
    }

    /// Indicates whether this [`Room`] reference is the same as the given
    /// [`Room`] reference. Compares pointers, not values.
    #[inline]
    #[must_use]
    pub fn ptr_eq(&self, other: &Room) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Checks [`RoomHandle`] equality by comparing inner pointers.
    #[inline]
    #[must_use]
    pub fn inner_ptr_eq(&self, handle: &RoomHandle) -> bool {
        handle
            .0
            .upgrade()
            .map_or(false, |handle_inner| Rc::ptr_eq(&self.0, &handle_inner))
    }

    /// Downgrades this [`Room`] to a weak reference.
    #[inline]
    #[must_use]
    pub fn downgrade(&self) -> WeakRoom {
        WeakRoom(Rc::downgrade(&self.0))
    }
}

/// Actual data of a [`Room`].
///
/// Shared between an external [`RoomHandle`] and Rust side ([`Room`]).
struct InnerRoom {
    /// Client to talk with media server via Client API RPC.
    rpc: Rc<dyn RpcSession>,

    /// Constraints to local [`local::Track`]s that are being published by
    /// [`PeerConnection`]s in this [`Room`].
    send_constraints: LocalTracksConstraints,

    /// Constraints to the [`remote::Track`] received by [`PeerConnection`]s
    /// in this [`Room`]. Used to disable or enable media receiving.
    recv_constraints: Rc<RecvConstraints>,

    /// [`peer::Component`]s repository.
    peers: peer::repo::Component,

    /// [`MediaManager`] for pre-obtaining [`local::Track`]s.
    media_manager: Rc<MediaManager>,

    /// Collection of [`Connection`]s with a remote `Member`s.
    ///
    /// [`Connection`]: crate::connection::Connection
    connections: Rc<Connections>,

    /// Callback invoked when a new local [`local::LocalMediaTrack`] will be
    /// added to this [`Room`].
    on_local_track: platform::Callback<api::LocalMediaTrack>,

    /// Callback invoked when failed obtain [`local::Track`]s from
    /// [`MediaManager`] or failed inject stream into [`PeerConnection`].
    on_failed_local_media: Rc<platform::Callback<api::Error>>,

    /// Callback invoked when a [`RpcSession`] loses connection.
    on_connection_loss: platform::Callback<api::ReconnectHandle>,

    /// Callback invoked when this [`Room`] is closed.
    on_close: Rc<platform::Callback<api::RoomCloseReason>>,

    /// Reason of [`Room`] closing.
    ///
    /// This [`CloseReason`] will be provided into [`RoomHandle::on_close`]
    /// callback.
    ///
    /// Note that `None` will be considered as error and `is_err` will be
    /// `true` in [`CloseReason`] provided to callback.
    close_reason: RefCell<CloseReason>,
}

/// Errors occurring in [`RoomHandle::set_local_media_settings()`] method.
#[derive(Debug, Display)]
pub enum ConstraintsUpdateError {
    /// New [`MediaStreamSettings`] set failed and state was recovered
    /// accordingly to the provided recover policy
    /// (`rollback_on_fail`/`stop_first` arguments).
    #[display(fmt = "RecoveredException")]
    Recovered(Traced<ChangeMediaStateError>),

    /// New [`MediaStreamSettings`] set failed and state recovering also
    /// failed.
    #[display(fmt = "RecoverFailedException")]
    RecoverFailed {
        /// [`ChangeMediaStateError`] due to which recovery has happened.
        recover_reason: Traced<ChangeMediaStateError>,

        /// [`ChangeMediaStateError`]s due to which recovery has failed.
        recover_fail_reasons: Vec<Traced<ChangeMediaStateError>>,
    },

    /// Some other error occurred.
    #[display(fmt = "ErroredException")]
    Errored(Traced<ChangeMediaStateError>),
}

impl ConstraintsUpdateError {
    /// Returns a name of this [`ConstraintsUpdateError`].
    #[inline]
    #[must_use]
    pub fn name(&self) -> String {
        self.to_string()
    }

    /// Returns a [`ChangeMediaStateError`] if this [`ConstraintsUpdateError`]
    /// represents a `RecoveredException` or a `RecoverFailedException`.
    #[inline]
    #[must_use]
    pub fn recover_reason(&self) -> Option<Traced<ChangeMediaStateError>> {
        match &self {
            Self::RecoverFailed { recover_reason, .. }
            | Self::Recovered(recover_reason) => Some(recover_reason.clone()),
            Self::Errored(_) => None,
        }
    }

    /// Returns a list of [`ChangeMediaStateError`]s due to which a recovery
    /// has failed.
    #[inline]
    #[must_use]
    pub fn recover_fail_reasons(&self) -> Vec<Traced<ChangeMediaStateError>> {
        match &self {
            Self::RecoverFailed {
                recover_fail_reasons,
                ..
            } => recover_fail_reasons.clone(),
            _ => Vec::new(),
        }
    }

    /// Returns a [`ChangeMediaStateError`] if this [`ConstraintsUpdateError`]
    /// represents an `ErroredException`.
    #[inline]
    #[must_use]
    pub fn error(&self) -> Option<Traced<ChangeMediaStateError>> {
        match &self {
            Self::Errored(error) => Some(error.clone()),
            _ => None,
        }
    }

    /// Returns a new [`ConstraintsUpdateError::Recovered`].
    #[inline]
    #[must_use]
    fn recovered(recover_reason: Traced<ChangeMediaStateError>) -> Self {
        Self::Recovered(recover_reason)
    }

    /// Converts this [`ChangeMediaStateError`] to the
    /// [`ConstraintsUpdateError::RecoverFailed`].
    #[must_use]
    fn recovery_failed(self, reason: Traced<ChangeMediaStateError>) -> Self {
        match self {
            Self::Recovered(recover_reason) => Self::RecoverFailed {
                recover_reason: reason,
                recover_fail_reasons: vec![recover_reason],
            },
            Self::RecoverFailed {
                recover_reason,
                mut recover_fail_reasons,
            } => {
                recover_fail_reasons.push(recover_reason);

                Self::RecoverFailed {
                    recover_reason: reason,
                    recover_fail_reasons,
                }
            }
            Self::Errored(error) => Self::RecoverFailed {
                recover_reason: error,
                recover_fail_reasons: vec![reason],
            },
        }
    }

    /// Returns a [`ConstraintsUpdateError::Errored`] with the provided
    /// [`ChangeMediaStateError`].
    #[inline]
    #[must_use]
    fn errored(err: Traced<ChangeMediaStateError>) -> Self {
        Self::Errored(err)
    }
}

impl InnerRoom {
    /// Creates a new [`InnerRoom`].
    #[inline]
    fn new(
        rpc: Rc<dyn RpcSession>,
        media_manager: Rc<MediaManager>,
        peer_event_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Self {
        let connections = Rc::new(Connections::default());
        let send_constraints = LocalTracksConstraints::default();
        let recv_constraints = Rc::new(RecvConstraints::default());
        Self {
            peers: peer::repo::Component::new(
                Rc::new(peer::repo::Repository::new(
                    Rc::clone(&media_manager),
                    peer_event_sender,
                    send_constraints.clone(),
                    Rc::clone(&recv_constraints),
                    Rc::clone(&connections),
                )),
                Rc::new(peer::repo::State::default()),
            ),
            media_manager,
            rpc,
            send_constraints,
            recv_constraints,
            connections,
            on_connection_loss: platform::Callback::default(),
            on_failed_local_media: Rc::new(platform::Callback::default()),
            on_local_track: platform::Callback::default(),
            on_close: Rc::new(platform::Callback::default()),
            close_reason: RefCell::new(CloseReason::ByClient {
                reason: ClientDisconnect::RoomUnexpectedlyDropped,
                is_err: true,
            }),
        }
    }

    /// Toggles [`InnerRoom::recv_constraints`] or
    /// [`InnerRoom::send_constraints`] media exchange status based on the
    /// provided [`TrackDirection`], [`MediaKind`] and
    /// [`proto::MediaSourceKind`].
    fn set_constraints_media_state(
        &self,
        state: MediaState,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<proto::MediaSourceKind>,
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

    /// Sets `close_reason` of this [`InnerRoom`].
    ///
    /// [`Drop`] implementation of [`InnerRoom`] is supposed to be triggered
    /// after this function call.
    fn set_close_reason(&self, reason: CloseReason) {
        self.close_reason.replace(reason);
    }

    /// Toggles [`TransceiverSide`]s [`MediaState`] by the provided
    /// [`MediaKind`] in all [`PeerConnection`]s of this [`Room`].
    ///
    /// [`TransceiverSide`]: crate::peer::TransceiverSide
    async fn toggle_media_state(
        &self,
        state: MediaState,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<proto::MediaSourceKind>,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        let tracks: HashMap<_, _> = self
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

        self.update_media_states(tracks).await
    }

    /// Updates [`MediaState`]s of the [`TransceiverSide`] with the provided
    /// [`PeerId`] and [`TrackId`] to the provided [`MediaState`]s.
    ///
    /// [`TransceiverSide`]: crate::peer::TransceiverSide
    async fn update_media_states(
        &self,
        desired_states: HashMap<PeerId, HashMap<TrackId, MediaState>>,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        let stream_upd_sub: HashMap<PeerId, HashSet<TrackId>> = desired_states
            .iter()
            .map(|(id, states)| {
                (
                    *id,
                    states
                        .iter()
                        .filter_map(|(id, state)| {
                            if matches!(
                                state,
                                MediaState::MediaExchange(
                                    media_exchange_state::Stable::Enabled
                                )
                            ) {
                                Some(*id)
                            } else {
                                None
                            }
                        })
                        .collect(),
                )
            })
            .collect();
        future::try_join_all(
            desired_states
                .into_iter()
                .filter_map(|(peer_id, desired_states)| {
                    self.peers.get(peer_id).map(|peer| (peer, desired_states))
                })
                .map(|(peer, desired_states)| {
                    let transitions_futs: Vec<_> = desired_states
                        .into_iter()
                        .filter_map(move |(track_id, desired_state)| {
                            peer.get_transceiver_side_by_id(track_id)
                                .map(|trnscvr| (trnscvr, desired_state))
                        })
                        .filter_map(|(trnscvr, desired_state)| {
                            if trnscvr.is_subscription_needed(desired_state) {
                                Some((trnscvr, desired_state))
                            } else {
                                None
                            }
                        })
                        .map(|(trnscvr, desired_state)| {
                            trnscvr.media_state_transition_to(desired_state)?;

                            Ok(trnscvr.when_media_state_stable(desired_state))
                        })
                        .collect::<Result<_, Traced<ProhibitedStateError>>>()
                        .map_err(tracerr::wrap!())?;

                    Ok(future::try_join_all(transitions_futs))
                })
                .collect::<Result<Vec<_>, Traced<ProhibitedStateError>>>()
                .map_err(tracerr::map_from_and_wrap!())?,
        )
        .await
        .map_err(tracerr::from_and_wrap!())?;

        future::try_join_all(stream_upd_sub.into_iter().filter_map(
            |(id, tracks_ids)| {
                Some(
                    self.peers
                        .state()
                        .get(id)?
                        .local_stream_update_result(tracks_ids),
                )
            },
        ))
        .map(|r| r.map(drop))
        .await
        .map_err(tracerr::map_from_and_wrap!())
        .map(drop)
    }

    /// Returns [`local::Track`]s for the provided [`MediaKind`] and
    /// [`proto::MediaSourceKind`].
    ///
    /// If [`proto::MediaSourceKind`] is [`None`] then [`local::Track`]s for all
    /// needed [`proto::MediaSourceKind`]s will be returned.
    ///
    /// # Errors
    ///
    /// With [`GetLocalTracksError::CouldNotGetLocalMedia`] if failed to obtain
    /// [`local::Track`]s from the [`MediaManager`].
    ///
    /// With [`GetLocalTracksError::InvalidLocalTracks`] if failed to get
    /// [`MediaStreamSettings`].
    async fn get_local_tracks(
        &self,
        kind: MediaKind,
        source_kind: Option<proto::MediaSourceKind>,
    ) -> Result<Vec<Rc<local::Track>>, Traced<GetLocalTracksError>> {
        let requests: Vec<_> = self
            .peers
            .get_all()
            .into_iter()
            .filter_map(|p| p.get_media_settings(kind, source_kind).transpose())
            .collect::<Result<Vec<_>, _>>()
            .map_err(tracerr::map_from_and_wrap!())?;

        let mut result = Vec::new();
        for req in requests {
            let tracks = self
                .media_manager
                .get_tracks(req)
                .await
                .map_err(|e| {
                    self.on_failed_local_media.call1(e.clone());

                    e
                })
                .map_err(tracerr::map_from_and_wrap!())?;
            for (track, is_new) in tracks {
                if is_new {
                    self.on_local_track
                        .call1(local::LocalMediaTrack::new(Rc::clone(&track)));
                }
                result.push(track);
            }
        }

        Ok(result)
    }

    /// Returns `true` if all [`Sender`]s or [`Receiver`]s with a provided
    /// [`MediaKind`] and [`proto::MediaSourceKind`] of this [`Room`] are in the
    /// provided [`MediaState`].
    ///
    /// [`Sender`]: peer::media::Sender
    /// [`Receiver`]: peer::media::Receiver
    pub fn is_all_peers_in_media_state(
        &self,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<proto::MediaSourceKind>,
        state: MediaState,
    ) -> bool {
        !self.peers.get_all().into_iter().any(|p| {
            !p.is_all_transceiver_sides_in_media_state(
                kind,
                direction,
                source_kind,
                state,
            )
        })
    }

    /// Updates [`MediaState`]s to the provided `states_update` and disables all
    /// [`Sender`]s which doesn't have [`local::Track`].
    ///
    /// [`Sender`]: peer::media::Sender
    async fn disable_senders_without_tracks(
        &self,
        peer: &Rc<PeerConnection>,
        kinds: LocalStreamUpdateCriteria,
        mut states_update: HashMap<PeerId, HashMap<TrackId, MediaState>>,
    ) -> Result<(), Traced<ChangeMediaStateError>> {
        use media_exchange_state::Stable::Disabled;

        self.send_constraints
            .set_media_exchange_state_by_kinds(Disabled, kinds);
        let senders_to_disable = peer.get_senders_without_tracks_ids(kinds);

        states_update.entry(peer.id()).or_default().extend(
            senders_to_disable
                .into_iter()
                .map(|id| (id, MediaState::from(Disabled))),
        );
        self.update_media_states(states_update)
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
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
    /// Will update [`media_exchange_state::Stable`]s of the [`Sender`]s that
    /// should be enabled or disabled.
    ///
    /// If `stop_first` set to `true` then affected [`local::Track`]s will be
    /// dropped before new [`MediaStreamSettings`] is applied. This is usually
    /// required when changing video source device due to hardware limitations,
    /// e.g. having an active track sourced from device `A` may hinder
    /// [getUserMedia()][1] requests to device `B`.
    ///
    /// `rollback_on_fail` option configures [`MediaStreamSettings`] update
    /// request to automatically rollback to previous settings if new settings
    /// cannot be applied.
    ///
    /// If recovering from fail state isn't possible and `stop_first` set to
    /// `true` then affected media types will be disabled.
    ///
    /// [`Sender`]: peer::media::Sender
    /// [1]: https://tinyurl.com/rnxcavf
    #[async_recursion(?Send)]
    async fn set_local_media_settings(
        &self,
        new_settings: MediaStreamSettings,
        stop_first: bool,
        rollback_on_fail: bool,
    ) -> Result<(), ConstraintsUpdateError> {
        use ConstraintsUpdateError as E;

        let current_settings = self.send_constraints.inner();
        self.send_constraints.constrain(new_settings);
        let criteria_kinds_diff = self
            .send_constraints
            .calculate_kinds_diff(&current_settings);
        let peers = self.peers.get_all();

        if stop_first {
            for peer in &peers {
                peer.drop_send_tracks(criteria_kinds_diff).await;
            }
        }

        let mut states_update: HashMap<_, HashMap<_, _>> = HashMap::new();
        for peer in peers {
            match peer
                .update_local_stream(LocalStreamUpdateCriteria::all())
                .await
            {
                Ok(states) => {
                    states_update.entry(peer.id()).or_default().extend(
                        states.into_iter().map(|(id, s)| (id, s.into())),
                    );
                }
                Err(e) => {
                    if !matches!(
                        e.as_ref(),
                        UpdateLocalStreamError::CouldNotGetLocalMedia(_)
                    ) {
                        return Err(E::errored(tracerr::map_from_and_wrap!()(
                            e.clone(),
                        )));
                    }

                    let err = if rollback_on_fail {
                        self.set_local_media_settings(
                            current_settings,
                            stop_first,
                            false,
                        )
                        .await
                        .map_err(|err| {
                            err.recovery_failed(tracerr::map_from_and_wrap!()(
                                e.clone(),
                            ))
                        })?;

                        E::recovered(tracerr::map_from_and_wrap!()(e.clone()))
                    } else if stop_first {
                        self.disable_senders_without_tracks(
                            &peer,
                            criteria_kinds_diff,
                            states_update,
                        )
                        .await
                        .map_err(|err| {
                            E::RecoverFailed {
                                recover_reason: tracerr::map_from_and_new!(
                                    e.clone()
                                ),
                                recover_fail_reasons: vec![
                                    tracerr::map_from_and_new!(err),
                                ],
                            }
                        })?;

                        E::recovered(tracerr::map_from_and_wrap!()(e.clone()))
                    } else {
                        E::errored(tracerr::map_from_and_wrap!()(e.clone()))
                    };

                    return Err(err);
                }
            }
        }

        self.update_media_states(states_update)
            .await
            .map_err(|e| E::errored(tracerr::map_from_and_new!(e)))
    }

    /// Stops state transition timers in all [`PeerConnection`]'s in this
    /// [`Room`].
    fn handle_rpc_connection_lost(&self) {
        self.peers.connection_lost();
        self.on_connection_loss
            .call1(ReconnectHandle::new(Rc::downgrade(&self.rpc)));
    }

    /// Sends [`Command::SynchronizeMe`] with a current Client state to the
    /// Media Server.
    ///
    /// Resets state transition timers in all [`PeerConnection`]'s in this
    /// [`Room`].
    fn handle_rpc_connection_recovered(&self) {
        self.peers.connection_recovered();
        self.rpc.send_command(Command::SynchronizeMe {
            state: self.peers.state().as_proto(),
        });
    }
}

/// Error of a [`RpcEvent`] containing a [`PeerId`] that a [`Room`] is not aware
/// of.
#[derive(Clone, Copy, Debug, Display)]
#[display(fmt = "Peer with id {} doesnt exist", _0)]
struct UnknownPeerIdError(PeerId);

/// RPC events handling.
#[async_trait(?Send)]
impl EventHandler for InnerRoom {
    type Output = Result<(), Traced<UnknownPeerIdError>>;

    /// Creates [`PeerConnection`] with a provided ID and all the
    /// [`Connection`]s basing on provided [`Track`]s.
    ///
    /// If provided `sdp_offer` is `Some`, then offer is applied to a created
    /// peer, and [`Command::MakeSdpAnswer`] is emitted back to the RPC server.
    ///
    /// [`Connection`]: crate::connection::Connection
    async fn on_peer_created(
        &self,
        peer_id: PeerId,
        negotiation_role: NegotiationRole,
        tracks: Vec<Track>,
        ice_servers: Vec<IceServer>,
        is_force_relayed: bool,
    ) -> Self::Output {
        let peer_state = peer::State::new(
            peer_id,
            ice_servers,
            is_force_relayed,
            Some(negotiation_role),
        );
        for track in &tracks {
            peer_state.insert_track(track, self.send_constraints.clone());
        }

        self.peers.state().insert(peer_id, peer_state);

        Ok(())
    }

    /// Applies specified SDP Answer to a specified [`PeerConnection`].
    async fn on_sdp_answer_made(
        &self,
        peer_id: PeerId,
        sdp_answer: String,
    ) -> Self::Output {
        let peer = self
            .peers
            .state()
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(UnknownPeerIdError(peer_id)))?;
        peer.set_remote_sdp(sdp_answer);

        Ok(())
    }

    /// Applies provided SDP to the [`peer::State`] with a provided [`PeerId`].
    async fn on_local_description_applied(
        &self,
        peer_id: PeerId,
        local_sdp: String,
    ) -> Self::Output {
        let peer_state = self
            .peers
            .state()
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(UnknownPeerIdError(peer_id)))?;
        peer_state.apply_local_sdp(local_sdp);

        Ok(())
    }

    /// Applies specified [`IceCandidate`] to a specified [`PeerConnection`].
    async fn on_ice_candidate_discovered(
        &self,
        peer_id: PeerId,
        candidate: IceCandidate,
    ) -> Self::Output {
        let peer = self
            .peers
            .state()
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(UnknownPeerIdError(peer_id)))?;
        peer.add_ice_candidate(candidate);

        Ok(())
    }

    /// Disposes specified [`PeerConnection`]s.
    async fn on_peers_removed(&self, peer_ids: Vec<PeerId>) -> Self::Output {
        for id in peer_ids {
            self.peers.state().remove(id);
        }
        Ok(())
    }

    /// Creates new `Track`s, updates existing [`Sender`]s/[`Receiver`]s with
    /// [`PeerUpdate`]s.
    ///
    /// Will start (re)negotiation process if `Some` [`NegotiationRole`] is
    /// provided.
    ///
    /// [`Receiver`]: peer::media::Receiver
    /// [`Sender`]: peer::media::Sender
    async fn on_peer_updated(
        &self,
        peer_id: PeerId,
        updates: Vec<PeerUpdate>,
        negotiation_role: Option<NegotiationRole>,
    ) -> Self::Output {
        let peer_state = self
            .peers
            .state()
            .get(peer_id)
            .ok_or_else(|| tracerr::new!(UnknownPeerIdError(peer_id)))?;

        for update in updates {
            match update {
                PeerUpdate::Added(track) => peer_state
                    .insert_track(&track, self.send_constraints.clone()),
                PeerUpdate::Updated(patch) => peer_state.patch_track(&patch),
                PeerUpdate::IceRestart => {
                    peer_state.restart_ice();
                }
                PeerUpdate::Removed(id) => {
                    peer_state.remove_track(id);
                }
            }
        }
        if let Some(negotiation_role) = negotiation_role {
            peer_state.set_negotiation_role(negotiation_role).await;
        }

        Ok(())
    }

    /// Updates [`Connection`]'s [`ConnectionQualityScore`] by calling
    /// [`Connection::update_quality_score()`][1].
    ///
    /// [`Connection`]: crate::connection::Connection
    /// [1]: crate::connection::Connection::update_quality_score
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

    /// Updates [`peer::repo::State`] with the provided [`proto::state::Room`].
    #[inline]
    async fn on_state_synchronized(
        &self,
        state: proto::state::Room,
    ) -> Self::Output {
        self.peers.apply(state);
        Ok(())
    }
}

/// Error of a [`PeerEvent::NewRemoteTrack`] containing an unknown remote
/// [`MemberId`].
#[derive(Clone, Debug, Display)]
#[display(fmt = "Remote stream from unknown member")]
struct UnknownRemoteMemberError(MemberId);

/// [`PeerEvent`]s handling.
#[async_trait(?Send)]
impl PeerEventHandler for InnerRoom {
    type Output = Result<(), Traced<UnknownRemoteMemberError>>;

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
    /// [`Connection`]: crate::connection::Connection
    /// [`Stream`]: futures::Stream
    async fn on_new_remote_track(
        &self,
        sender_id: MemberId,
        track: remote::Track,
    ) -> Self::Output {
        let conn = self.connections.get(&sender_id).ok_or_else(|| {
            tracerr::new!(UnknownRemoteMemberError(sender_id))
        })?;
        conn.add_remote_track(track);

        Ok(())
    }

    /// Invokes `on_local_track` [`Room`]'s callback.
    async fn on_new_local_track(
        &self,
        track: Rc<local::Track>,
    ) -> Self::Output {
        self.on_local_track
            .call1(local::LocalMediaTrack::new(track));
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
        stats: platform::RtcStats,
    ) -> Self::Output {
        self.rpc.send_command(Command::AddPeerConnectionMetrics {
            peer_id,
            metrics: PeerMetrics::RtcStats(stats.0),
        });
        Ok(())
    }

    /// Handles [`PeerEvent::FailedLocalMedia`] event by invoking
    /// `on_failed_local_media` [`Room`]'s callback.
    async fn on_failed_local_media(
        &self,
        error: Traced<LocalMediaError>,
    ) -> Self::Output {
        self.on_failed_local_media.call1(api::Error::from(error));
        Ok(())
    }

    /// Handles [`PeerEvent::NewSdpOffer`] event by sending
    /// [`Command::MakeSdpOffer`] to the Media Server.
    async fn on_new_sdp_offer(
        &self,
        peer_id: PeerId,
        sdp_offer: String,
        mids: HashMap<TrackId, String>,
        transceivers_statuses: HashMap<TrackId, bool>,
    ) -> Self::Output {
        self.rpc.send_command(Command::MakeSdpOffer {
            peer_id,
            sdp_offer,
            mids,
            transceivers_statuses,
        });
        Ok(())
    }

    /// Handles [`PeerEvent::NewSdpAnswer`] event by sending
    /// [`Command::MakeSdpAnswer`] to the Media Server.
    async fn on_new_sdp_answer(
        &self,
        peer_id: PeerId,
        sdp_answer: String,
        transceivers_statuses: HashMap<TrackId, bool>,
    ) -> Self::Output {
        self.rpc.send_command(Command::MakeSdpAnswer {
            peer_id,
            sdp_answer,
            transceivers_statuses,
        });
        Ok(())
    }

    /// Handles [`PeerEvent::MediaUpdateCommand`] event by sending the provided
    /// [`Command`] to Media Server.
    async fn on_media_update_command(
        &self,
        intention: Command,
    ) -> Self::Output {
        self.rpc.send_command(intention);
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

        self.on_close
            .call1(RoomCloseReason::new(*self.close_reason.borrow()));
    }
}

#[cfg(feature = "mockable")]
impl Room {
    /// Returns [`PeerConnection`] stored in repository by its ID.
    ///
    /// Used to inspect [`Room`]'s inner state in integration tests.
    #[inline]
    pub fn get_peer_by_id(
        &self,
        peer_id: PeerId,
    ) -> Option<Rc<PeerConnection>> {
        self.0.peers.get(peer_id)
    }

    /// Returns reference to the [`peer::repo::State`] of this [`Room`].
    #[inline]
    pub fn peers_state(&self) -> Rc<peer::repo::State> {
        self.0.peers.state()
    }

    /// Lookups [`peer::State`] by the provided [`PeerId`].
    #[inline]
    pub fn get_peer_state_by_id(
        &self,
        peer_id: PeerId,
    ) -> Option<Rc<peer::State>> {
        self.0.peers.state().get(peer_id)
    }
}
