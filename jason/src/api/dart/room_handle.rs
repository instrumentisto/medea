use std::{convert::TryFrom as _, ptr};

use dart_sys::Dart_Handle;
use tracerr::Traced;

use crate::{
    api::dart::{
        utils::{
            c_str_into_string, DartFuture, DartResult, FormatException,
            InternalException, IntoDartFuture as _,
            MediaSettingsUpdateException, MediaStateTransitionException,
            StateError,
        },
        DartValueArg, ForeignClass,
    },
    media::MediaSourceKind,
    peer::{
        media::sender::CreateError, InsertLocalTracksError, LocalMediaError,
        UpdateLocalStreamError,
    },
    platform,
    room::{
        ChangeMediaStateError, ConstraintsUpdateError, HandleDetachedError,
        RoomJoinError,
    },
};

use super::{utils::DartError, MediaStreamSettings};

#[cfg(feature = "mockable")]
pub use self::mock::RoomHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::room::RoomHandle;

impl ForeignClass for RoomHandle {}

impl From<Traced<HandleDetachedError>> for DartError {
    #[inline]
    fn from(_: Traced<HandleDetachedError>) -> Self {
        StateError::new("ReconnectHandle is in detached state.").into()
    }
}

impl From<Traced<RoomJoinError>> for DartError {
    #[inline]
    fn from(err: Traced<RoomJoinError>) -> Self {
        let (err, trace) = err.into_parts();
        let message = err.to_string();

        match err {
            RoomJoinError::Detached | RoomJoinError::CallbackNotSet(_) => {
                StateError::new(message).into()
            }
            RoomJoinError::ConnectionInfoParse(_) => {
                FormatException::new(message).into()
            }
            RoomJoinError::SessionError(err) => {
                Traced::from_parts(err, trace).into()
            }
        }
    }
}

impl From<Traced<LocalMediaError>> for DartError {
    fn from(err: Traced<LocalMediaError>) -> Self {
        use InsertLocalTracksError as IE;
        use LocalMediaError as ME;
        use UpdateLocalStreamError as UE;

        let (err, trace) = err.into_parts();
        let message = err.to_string();

        match err {
            ME::UpdateLocalStreamError(err) => match err {
                UE::CouldNotGetLocalMedia(err) => {
                    Traced::from_parts(err, trace).into()
                }
                UE::InvalidLocalTracks(_)
                | UE::InsertLocalTracksError(
                    IE::InvalidMediaTrack | IE::NotEnoughTracks,
                ) => MediaStateTransitionException::new(message, trace).into(),
                UE::InsertLocalTracksError(IE::CouldNotInsertLocalTrack(_)) => {
                    InternalException::new(message, None, trace).into()
                }
            },
            ME::SenderCreateError(CreateError::TransceiverNotFound(_)) => {
                InternalException::new(message, None, trace).into()
            }
            ME::SenderCreateError(CreateError::CannotDisableRequiredSender) => {
                MediaStateTransitionException::new(message, trace).into()
            }
        }
    }
}

impl From<Traced<ChangeMediaStateError>> for DartError {
    #[inline]
    fn from(err: Traced<ChangeMediaStateError>) -> Self {
        let (err, trace) = err.into_parts();
        let message = err.to_string();

        match err {
            ChangeMediaStateError::Detached => {
                StateError::new("RoomHandle is in detached state.").into()
            }
            ChangeMediaStateError::CouldNotGetLocalMedia(err) => {
                Traced::from_parts(err, trace).into()
            }
            ChangeMediaStateError::ProhibitedState(_)
            | ChangeMediaStateError::TransitionIntoOppositeState(_)
            | ChangeMediaStateError::InvalidLocalTracks(_) => {
                MediaStateTransitionException::new(message, trace).into()
            }
            ChangeMediaStateError::InsertLocalTracksError(_) => {
                InternalException::new(message, None, trace).into()
            }
        }
    }
}

impl From<ConstraintsUpdateError> for DartError {
    #[inline]
    fn from(err: ConstraintsUpdateError) -> Self {
        let message = err.to_string();

        let (err, rolled_back) = match err {
            ConstraintsUpdateError::Recovered(err) => (err, true),
            ConstraintsUpdateError::RecoverFailed {
                recover_reason, ..
            } => (recover_reason, false),
            ConstraintsUpdateError::Errored(err) => (err, false),
        };

        MediaSettingsUpdateException::new(message, err, rolled_back).into()
    }
}

/// Connects to a media server and joins the [`Room`] with the provided
/// authorization `token`.
///
/// Authorization token has a fixed format:
/// `{{ Host URL }}/{{ Room ID }}/{{ Member ID }}?token={{ Auth Token }}`
/// (e.g. `wss://medea.com/MyConf1/Alice?token=777`).
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__join(
    this: ptr::NonNull<RoomHandle>,
    token: ptr::NonNull<libc::c_char>,
) -> DartFuture<Result<(), Traced<RoomJoinError>>> {
    let this = this.as_ref().clone();

    async move {
        this.join(c_str_into_string(token)).await?;
        Ok(())
    }
    .into_dart_future()
}

/// Updates this [`Room`]'s [`MediaStreamSettings`]. This affects all the
/// [`PeerConnection`]s in this [`Room`]. If [`MediaStreamSettings`] are
/// configured for some [`Room`], then this [`Room`] can only send media tracks
/// that correspond to these settings. [`MediaStreamSettings`] update will
/// change media tracks in all sending peers, so that might cause a new
/// [getUserMedia()][1] request to happen.
///
/// Media obtaining/injection errors are additionally fired to
/// `on_failed_local_media` callback.
///
/// If `stop_first` set to `true` then affected local `Tracks` will be
/// dropped before new [`MediaStreamSettings`] are applied. This is usually
/// required when changing video source device due to hardware limitations,
/// e.g. having an active track sourced from device `A` may hinder
/// [getUserMedia()][1] requests to device `B`.
///
/// `rollback_on_fail` option configures [`MediaStreamSettings`] update request
/// to automatically rollback to previous settings if new settings cannot be
/// applied.
///
/// If recovering from fail state isn't possible then affected media types will
/// be disabled.
///
/// [`Room`]: crate::room::Room
/// [`PeerConnection`]: crate::peer::PeerConnection
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__set_local_media_settings(
    this: ptr::NonNull<RoomHandle>,
    settings: ptr::NonNull<MediaStreamSettings>,
    stop_first: bool,
    rollback_on_fail: bool,
) -> DartFuture<Result<(), ConstraintsUpdateError>> {
    let this = this.as_ref().clone();
    let settings = settings.as_ref().clone();

    async move {
        this.set_local_media_settings(settings, stop_first, rollback_on_fail)
            .await?;
        Ok(())
    }
    .into_dart_future()
}

/// Mutes outbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__mute_audio(
    this: ptr::NonNull<RoomHandle>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();

    async move {
        this.mute_audio().await?;
        Ok(())
    }
    .into_dart_future()
}

/// Unmutes outbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__unmute_audio(
    this: ptr::NonNull<RoomHandle>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();

    async move {
        this.mute_audio().await?;
        Ok(())
    }
    .into_dart_future()
}

/// Enables outbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_audio(
    this: ptr::NonNull<RoomHandle>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();

    async move {
        this.enable_audio().await?;
        Ok(())
    }
    .into_dart_future()
}

/// Disables outbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_audio(
    this: ptr::NonNull<RoomHandle>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();

    async move {
        this.disable_audio().await?;
        Ok(())
    }
    .into_dart_future()
}

/// Mutes outbound video in this [`Room`].
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__mute_video(
    this: ptr::NonNull<RoomHandle>,
    source_kind: DartValueArg<Option<MediaSourceKind>>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    // TODO: Remove unwraps when propagating fatal errors from Rust to Dart is
    //       implemented.
    let this = this.as_ref().clone();
    let source_kind = Option::<i64>::try_from(source_kind)
        .unwrap()
        .map(MediaSourceKind::from);

    async move {
        this.mute_video(source_kind).await?;
        Ok(())
    }
    .into_dart_future()
}

/// Unmutes outbound video in this [`Room`].
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__unmute_video(
    this: ptr::NonNull<RoomHandle>,
    source_kind: DartValueArg<Option<MediaSourceKind>>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();
    // TODO: Remove unwraps when propagating fatal errors from Rust to Dart is
    //       implemented.
    let source_kind = Option::<i64>::try_from(source_kind)
        .unwrap()
        .map(MediaSourceKind::from);

    async move {
        this.unmute_video(source_kind).await?;
        Ok(())
    }
    .into_dart_future()
}

/// Enables outbound video.
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_video(
    this: ptr::NonNull<RoomHandle>,
    source_kind: DartValueArg<Option<MediaSourceKind>>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();
    // TODO: Remove unwraps when propagating fatal errors from Rust to Dart is
    //       implemented.
    let source_kind = Option::<i64>::try_from(source_kind)
        .unwrap()
        .map(MediaSourceKind::from);

    async move {
        this.enable_video(source_kind).await?;
        Ok(())
    }
    .into_dart_future()
}

/// Disables outbound video.
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_video(
    this: ptr::NonNull<RoomHandle>,
    source_kind: DartValueArg<Option<MediaSourceKind>>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    // TODO: Remove unwraps when propagating fatal errors from Rust to Dart is
    //       implemented.
    let this = this.as_ref().clone();
    let source_kind = Option::<i64>::try_from(source_kind)
        .unwrap()
        .map(MediaSourceKind::from);

    async move {
        this.disable_video(source_kind).await?;
        Ok(())
    }
    .into_dart_future()
}

/// Enables inbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_remote_audio(
    this: ptr::NonNull<RoomHandle>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();

    async move {
        this.enable_remote_audio().await?;
        Ok(())
    }
    .into_dart_future()
}

/// Disables inbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_remote_audio(
    this: ptr::NonNull<RoomHandle>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();

    async move {
        this.disable_remote_audio().await?;
        Ok(())
    }
    .into_dart_future()
}

/// Enables inbound video in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_remote_video(
    this: ptr::NonNull<RoomHandle>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();

    async move {
        this.enable_remote_video().await?;
        Ok(())
    }
    .into_dart_future()
}

/// Disables inbound video in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_remote_video(
    this: ptr::NonNull<RoomHandle>,
) -> DartFuture<Result<(), Traced<ChangeMediaStateError>>> {
    let this = this.as_ref().clone();

    async move {
        this.disable_remote_video().await?;
        Ok(())
    }
    .into_dart_future()
}

/// Sets callback, invoked when a new [`Connection`] with some remote `Peer`
/// is established.
///
/// [`Connection`]: crate::connection::Connection
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_new_connection(
    this: ptr::NonNull<RoomHandle>,
    cb: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_new_connection(platform::Function::new(cb))
        .map_err(DartError::from)
        .into()
}

/// Sets callback, invoked on this [`Room`] close, providing a
/// [`RoomCloseReason`].
///
/// [`Room`]: crate::room::Room
/// [`RoomCloseReason`]: crate::room::RoomCloseReason
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_close(
    this: ptr::NonNull<RoomHandle>,
    cb: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_close(platform::Function::new(cb))
        .map_err(DartError::from)
        .into()
}

/// Sets callback, invoked when a new [`LocalMediaTrack`] is added to this
/// [`Room`].
///
/// This might happen in such cases:
/// 1. Media server initiates a media request.
/// 2. `enable_audio`/`enable_video` is called.
/// 3. [`MediaStreamSettings`] updated via `set_local_media_settings`.
///
/// [`Room`]: crate::room::Room
/// [`MediaStreamSettings`]: crate::media::MediaStreamSettings
/// [`LocalMediaTrack`]: crate::media::track::local::LocalMediaTrack
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_local_track(
    this: ptr::NonNull<RoomHandle>,
    cb: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_local_track(platform::Function::new(cb))
        .map_err(DartError::from)
        .into()
}

/// Sets callback, invoked when a connection with server is lost.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_connection_loss(
    this: ptr::NonNull<RoomHandle>,
    cb: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_connection_loss(platform::Function::new(cb))
        .map_err(DartError::from)
        .into()
}

/// Sets callback, invoked on local media acquisition failures.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_failed_local_media(
    this: ptr::NonNull<RoomHandle>,
    cb: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_failed_local_media(platform::Function::new(cb))
        .map_err(DartError::from)
        .into()
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__free(this: ptr::NonNull<RoomHandle>) {
    drop(RoomHandle::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use tracerr::Traced;

    use crate::{
        api::{
            dart::utils::DartError, ConnectionHandle, LocalMediaTrack,
            MediaStreamSettings, ReconnectHandle,
        },
        media::MediaSourceKind,
        peer::{LocalMediaError, TracksRequestError, UpdateLocalStreamError},
        platform,
        room::{
            ChangeMediaStateError, ConstraintsUpdateError, HandleDetachedError,
            RoomCloseReason, RoomJoinError,
        },
        rpc::{ClientDisconnect, CloseReason, ConnectionInfo},
    };

    #[derive(Clone)]
    pub struct RoomHandle;

    #[allow(clippy::missing_errors_doc)]
    impl RoomHandle {
        pub fn on_new_connection(
            &self,
            cb: platform::Function<ConnectionHandle>,
        ) -> Result<(), Traced<HandleDetachedError>> {
            cb.call1(ConnectionHandle);
            Ok(())
        }

        pub fn on_close(
            &self,
            cb: platform::Function<RoomCloseReason>,
        ) -> Result<(), Traced<HandleDetachedError>> {
            cb.call1(RoomCloseReason::new(CloseReason::ByClient {
                is_err: true,
                reason: ClientDisconnect::RpcClientUnexpectedlyDropped,
            }));
            Ok(())
        }

        pub fn on_local_track(
            &self,
            cb: platform::Function<LocalMediaTrack>,
        ) -> Result<(), Traced<HandleDetachedError>> {
            cb.call1(LocalMediaTrack);
            Ok(())
        }

        pub fn on_connection_loss(
            &self,
            cb: platform::Function<ReconnectHandle>,
        ) -> Result<(), Traced<HandleDetachedError>> {
            cb.call1(ReconnectHandle);
            Ok(())
        }

        pub async fn join(
            &self,
            token: String,
        ) -> Result<(), Traced<RoomJoinError>> {
            token
                .parse::<ConnectionInfo>()
                .map_err(tracerr::map_from_and_wrap!())
                .map(drop)
        }

        pub fn on_failed_local_media(
            &self,
            cb: platform::Function<DartError>,
        ) -> Result<(), Traced<HandleDetachedError>> {
            let err = LocalMediaError::UpdateLocalStreamError(
                UpdateLocalStreamError::InvalidLocalTracks(
                    TracksRequestError::NoTracks,
                ),
            );
            cb.call1(tracerr::new!(err).into());
            Ok(())
        }

        pub async fn set_local_media_settings(
            &self,
            _settings: MediaStreamSettings,
            _stop_first: bool,
            _rollback_on_fail: bool,
        ) -> Result<(), ConstraintsUpdateError> {
            Ok(())
        }

        pub async fn mute_audio(
            &self,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            Ok(())
        }

        pub async fn unmute_audio(
            &self,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            Ok(())
        }

        pub async fn enable_audio(
            &self,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            Ok(())
        }

        pub async fn disable_audio(
            &self,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            Ok(())
        }

        pub async fn mute_video(
            &self,
            source_kind: Option<MediaSourceKind>,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            assert_eq!(source_kind, None);
            Ok(())
        }

        pub async fn unmute_video(
            &self,
            source_kind: Option<MediaSourceKind>,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            assert_eq!(source_kind, Some(MediaSourceKind::Display));
            Ok(())
        }

        pub async fn enable_video(
            &self,
            source_kind: Option<MediaSourceKind>,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            assert_eq!(source_kind, Some(MediaSourceKind::Device));
            Ok(())
        }

        pub async fn disable_video(
            &self,
            source_kind: Option<MediaSourceKind>,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            assert_eq!(source_kind, Some(MediaSourceKind::Display));
            Ok(())
        }

        pub async fn enable_remote_audio(
            &self,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            Ok(())
        }

        pub async fn disable_remote_audio(
            &self,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            Ok(())
        }

        pub async fn enable_remote_video(
            &self,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            Err(tracerr::new!(ChangeMediaStateError::Detached).into())
        }

        pub async fn disable_remote_video(
            &self,
        ) -> Result<(), Traced<ChangeMediaStateError>> {
            Ok(())
        }
    }
}
