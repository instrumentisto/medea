use dart_sys::Dart_Handle;

use crate::{
    api::dart::{
        utils::{c_str_into_string, future_to_dart},
        ForeignClass,
    },
    media::MediaSourceKind,
    platform,
};

use super::MediaStreamSettings;

#[cfg(feature = "mockable")]
pub use self::mock::RoomHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::room::RoomHandle;

impl ForeignClass for RoomHandle {}

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
    this: *mut RoomHandle,
    token: *const libc::c_char,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.join(c_str_into_string(token)).await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Updates this [`Room`]'s [`MediaStreamSettings`]. This affects all
/// [`PeerConnection`]s in this [`Room`]. If [`MediaStreamSettings`] is
/// configured for some [`Room`], then this [`Room`] can only send media tracks
/// that correspond to this settings. [`MediaStreamSettings`] update will change
/// media tracks in all sending peers, so that might cause new
/// [getUserMedia()][1] request.
///
/// Media obtaining/injection errors are additionally fired to
/// `on_failed_local_media` callback.
///
/// If `stop_first` set to `true` then affected local `Tracks` will be
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
/// [`Room`]: crate::room::Room
/// [`PeerConnection`]: crate::peer::PeerConnection
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__set_local_media_settings(
    this: *mut RoomHandle,
    settings: *mut MediaStreamSettings,
    stop_first: bool,
    rollback_on_fail: bool,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();
    let settings = settings.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.set_local_media_settings(settings, stop_first, rollback_on_fail)
            .await
            .unwrap();
        Ok::<_, ()>(())
    })
}

/// Mutes outbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__mute_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.mute_audio().await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Unmutes outbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__unmute_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.mute_audio().await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Disables outbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.disable_audio().await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Enables outbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.enable_audio().await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Mutes outbound video in this [`Room`].
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__mute_video(
    this: *mut RoomHandle,
    source_kind: u8, // TODO: `source_kind` might be None.
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();
    let source_kind = MediaSourceKind::from(source_kind);

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.mute_video(Some(source_kind)).await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Unmutes outbound video in this [`Room`].
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__unmute_video(
    this: *mut RoomHandle,
    source_kind: u8, // TODO: `source_kind` might be None.
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();
    let source_kind = MediaSourceKind::from(source_kind);

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.unmute_video(Some(source_kind)).await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Disables outbound video.
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_video(
    this: *mut RoomHandle,
    source_kind: u8, // TODO: `source_kind` might be None.
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();
    let source_kind = MediaSourceKind::from(source_kind);

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.disable_video(Some(source_kind)).await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Enables outbound video.
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_video(
    this: *mut RoomHandle,
    source_kind: u8, // TODO: `source_kind` might be None.
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();
    let source_kind = MediaSourceKind::from(source_kind);

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.enable_video(Some(source_kind)).await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Enables inbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_remote_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.enable_remote_audio().await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Disables inbound audio in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_remote_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.disable_remote_audio().await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Disables inbound video in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_remote_video(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.disable_remote_video().await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Enables inbound video in this [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_remote_video(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap().clone();

    future_to_dart(async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.enable_remote_video().await.unwrap();
        Ok::<_, ()>(())
    })
}

/// Sets callback, invoked when a new [`Connection`] with some remote `Peer`
/// is established.
///
/// [`Connection`]: crate::connection::Connection
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_new_connection(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();

    // TODO: Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    this.on_new_connection(platform::Function::new(cb)).unwrap();
}

/// Sets callback, invoked on this [`Room`] close, providing a
/// [`RoomCloseReason`].
///
/// [`Room`]: crate::room::Room
/// [`RoomCloseReason`]: crate::room::RoomCloseReason
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_close(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();

    // TODO: Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    this.on_close(platform::Function::new(cb)).unwrap();
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
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();

    // TODO: Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    this.on_local_track(platform::Function::new(cb)).unwrap();
}

/// Sets callback, invoked when a connection with server is lost.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_connection_loss(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();

    // TODO: Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    this.on_connection_loss(platform::Function::new(cb))
        .unwrap();
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__free(this: *mut RoomHandle) {
    let _ = RoomHandle::from_ptr(this);
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::{
        api::{
            ConnectionHandle, JasonError, LocalMediaTrack, MediaStreamSettings,
            ReconnectHandle,
        },
        media::MediaSourceKind,
        platform,
        room::RoomCloseReason,
        rpc::{ClientDisconnect, CloseReason},
    };

    pub struct RoomHandle;

    #[allow(clippy::missing_errors_doc)]
    impl RoomHandle {
        pub fn on_new_connection(
            &self,
            cb: platform::Function<ConnectionHandle>,
        ) -> Result<(), JasonError> {
            cb.call1(ConnectionHandle);
            Ok(())
        }

        pub fn on_close(
            &self,
            cb: platform::Function<RoomCloseReason>,
        ) -> Result<(), JasonError> {
            cb.call1(RoomCloseReason::new(CloseReason::ByClient {
                is_err: true,
                reason: ClientDisconnect::RpcClientUnexpectedlyDropped,
            }));
            Ok(())
        }

        pub fn on_local_track(
            &self,
            cb: platform::Function<LocalMediaTrack>,
        ) -> Result<(), JasonError> {
            cb.call1(LocalMediaTrack);
            Ok(())
        }

        pub fn on_connection_loss(
            &self,
            cb: platform::Function<ReconnectHandle>,
        ) -> Result<(), JasonError> {
            cb.call1(ReconnectHandle);
            Ok(())
        }

        pub async fn join(&self, _token: String) -> Result<(), JasonError> {
            Ok(())
        }

        // pub fn on_failed_local_media(
        //     &self,
        //     f: Callback<JasonError>,
        // ) {
        //     // Result<(), JasonError>
        // }

        pub async fn set_local_media_settings(
            &self,
            _settings: MediaStreamSettings,
            _stop_first: bool,
            _rollback_on_fail: bool,
        ) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn mute_audio(&self) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn unmute_audio(&self) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn mute_video(
            &self,
            _source_kind: Option<MediaSourceKind>,
        ) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn unmute_video(
            &self,
            _source_kind: Option<MediaSourceKind>,
        ) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn disable_audio(&self) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn enable_audio(&self) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn disable_video(
            &self,
            _source_kind: Option<MediaSourceKind>,
        ) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn enable_video(
            &self,
            _source_kind: Option<MediaSourceKind>,
        ) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn disable_remote_audio(&self) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn disable_remote_video(&self) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn enable_remote_audio(&self) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn enable_remote_video(&self) -> Result<(), JasonError> {
            Ok(())
        }
    }
}
