use dart_sys::Dart_Handle;

use crate::{
    connection_handle::ConnectionHandle,
    local_media_track::LocalMediaTrack,
    media_stream_settings::MediaStreamSettings,
    reconnect_handle::ReconnectHandle,
    room_close_reason::RoomCloseReason,
    utils::{c_str_into_string, future_to_dart, DartClosure},
    ForeignClass, MediaSourceKind,
};

pub struct RoomHandle;

impl ForeignClass for RoomHandle {}

impl RoomHandle {
    pub fn on_new_connection(&self, cb: DartClosure<ConnectionHandle>) {
        // Result<(), JasonError>
        cb.call1(ConnectionHandle);
    }

    pub fn on_close(&self, cb: DartClosure<RoomCloseReason>) {
        // Result<(), JasonError>
        cb.call1(RoomCloseReason);
    }

    pub fn on_local_track(&self, cb: DartClosure<LocalMediaTrack>) {
        // Result<(), JasonError>
        cb.call1(LocalMediaTrack);
    }

    pub fn on_connection_loss(&self, cb: DartClosure<ReconnectHandle>) {
        // Result<(), JasonError>
        cb.call1(ReconnectHandle);
    }

    pub async fn join(&self, _token: String) {
        // Result<(), JasonError>
    }

    // pub fn on_failed_local_media(
    //     &self,
    //     f: Callback<JasonError>,
    // ) {
    //     // Result<(), JasonError>
    // }

    pub async fn set_local_media_settings(
        &self,
        _settings: &MediaStreamSettings,
        _stop_first: bool,
        _rollback_on_fail: bool,
    ) {
        // Result<(), ConstraintsUpdateException>
    }

    pub async fn mute_audio(&self) {
        // Result<(), JasonError>
    }

    pub async fn unmute_audio(&self) {
        // Result<(), JasonError>
    }

    pub async fn mute_video(&self, _source_kind: Option<MediaSourceKind>) {
        // Result<(), JasonError>
    }

    pub async fn unmute_video(&self, _source_kind: Option<MediaSourceKind>) {
        // Result<(), JasonError>
    }

    pub async fn disable_audio(&self) {
        // Result<(), JasonError>
    }

    pub async fn enable_audio(&self) {
        // Result<(), JasonError>
    }

    pub async fn disable_video(&self, _source_kind: Option<MediaSourceKind>) {
        // Result<(), JasonError>
    }

    pub async fn enable_video(&self, _source_kind: Option<MediaSourceKind>) {
        // Result<(), JasonError>
    }

    pub async fn disable_remote_audio(&self) {
        // Result<(), JasonError>
    }

    pub async fn disable_remote_video(&self) {
        // Result<(), JasonError>
    }

    pub async fn enable_remote_audio(&self) {
        // Result<(), JasonError>
    }

    pub async fn enable_remote_video(&self) {
        // Result<(), JasonError>
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
    this: *mut RoomHandle,
    token: *const libc::c_char,
) -> Dart_Handle {
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.join(c_str_into_string(token)).await;
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
    let this = this.as_ref().unwrap();
    let settings = MediaStreamSettings::from_ptr(settings);

    future_to_dart(async move {
        this.set_local_media_settings(&settings, stop_first, rollback_on_fail)
            .await;
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
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.mute_audio().await;
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
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.mute_audio().await;
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
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.disable_audio().await;
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
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.enable_audio().await;
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
    source_kind: i32,// TODO: `source_kind` might be None.
) -> Dart_Handle {
    let this = this.as_ref().unwrap();
    let source_kind = MediaSourceKind::from(source_kind);

    future_to_dart(async move {
        this.mute_video(Some(source_kind)).await;
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
    source_kind: i32, // TODO: `source_kind` might be None.
) -> Dart_Handle {
    let this = this.as_ref().unwrap();
    let source_kind = MediaSourceKind::from(source_kind);

    future_to_dart(async move {
        this.unmute_video(Some(source_kind)).await;
        Ok::<_, ()>(())
    })
}

/// Disables outbound video.
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_video(
    this: *mut RoomHandle,
    source_kind: i32, // TODO: `source_kind` might be None.
) -> Dart_Handle {
    let this = this.as_ref().unwrap();
    let source_kind = MediaSourceKind::from(source_kind);

    future_to_dart(async move {
        this.disable_video(Some(source_kind)).await;
        Ok::<_, ()>(())
    })
}

/// Enables outbound video.
///
/// Affects only video with specific [`MediaSourceKind`] if specified.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_video(
    this: *mut RoomHandle,
    source_kind: i32, // TODO: `source_kind` might be None.
) -> Dart_Handle {
    let this = this.as_ref().unwrap();
    let source_kind = MediaSourceKind::from(source_kind);

    future_to_dart(async move {
        this.enable_video(Some(source_kind)).await;
        Ok::<_, ()>(())
    })
}

/// Enables inbound audio in this [`Room`].
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_remote_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.enable_remote_audio().await;
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
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.disable_remote_audio().await;
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
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.disable_remote_video().await;
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
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.enable_remote_video().await;
        Ok::<_, ()>(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_new_connection(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_new_connection(DartClosure::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_close(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_close(DartClosure::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_local_track(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_local_track(DartClosure::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_connection_loss(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_connection_loss(DartClosure::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__free(this: *mut RoomHandle) {
    RoomHandle::from_ptr(this);
}
