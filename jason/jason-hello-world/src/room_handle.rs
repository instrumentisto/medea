use std::any::Any;

use dart_sys::Dart_Handle;

use crate::media_stream_settings::MediaStreamSettings;
use crate::{
    completer::Completer, connection_handle::ConnectionHandle,
    local_media_track::LocalMediaTrack, reconnect_handle::ReconnectHandle,
    room_close_reason::RoomCloseReason, DartCallback, MediaSourceKind,
};
use extern_executor::spawn;

pub struct RoomHandle;

impl RoomHandle {
    pub fn on_close(&self, cb: DartCallback<RoomCloseReason>) {}

    pub fn on_local_track(&self, cb: DartCallback<LocalMediaTrack>) {}

    // TODO: on_failed_local_stream

    pub fn on_connection_loss(&self, cb: DartCallback<ReconnectHandle>) {}

    pub fn on_new_connection(&self, cb: DartCallback<ConnectionHandle>) {}

    pub async fn join(&self) {}

    pub async fn set_local_media_settings(
        &self,
        settings: &MediaStreamSettings,
        stop_first: bool,
        rollback_on_fail: bool,
    ) {
    }

    pub async fn mute_audio(&self) {}
    pub async fn unmute_audio(&self) {}
    pub async fn disable_audio(&self) {}
    pub async fn enable_audio(&self) {}

    pub async fn mute_video(&self, source_kind: MediaSourceKind) {}
    pub async fn unmute_video(&self, source_kind: MediaSourceKind) {}
    pub async fn disable_video(&self, source_kind: MediaSourceKind) {}
    pub async fn enable_video(&self, source_kind: MediaSourceKind) {}

    pub async fn disable_remote_audio(&self) {}
    pub async fn enable_remote_audio(&self) {}
    pub async fn disable_remote_video(&self) {}
    pub async fn enable_remote_video(&self) {}
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__join(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    extern_executor::spawn(async move {
        this.join().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__set_local_media_settings(
    this: *mut RoomHandle,
    settings: *mut MediaStreamSettings,
    stop_first: bool,
    rollback_on_fail: bool,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let settings = Box::from_raw(settings);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.set_local_media_settings(&settings, stop_first, rollback_on_fail)
            .await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__mute_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.mute_audio().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__unmute_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.mute_audio().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.disable_audio().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.enable_audio().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__mute_video(
    this: *mut RoomHandle,
    source_kind: i32,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let source_kind = MediaSourceKind::from(source_kind);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.mute_video(source_kind).await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__unmute_video(
    this: *mut RoomHandle,
    source_kind: i32,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let source_kind = MediaSourceKind::from(source_kind);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.unmute_video(source_kind).await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_video(
    this: *mut RoomHandle,
    source_kind: i32,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let source_kind = MediaSourceKind::from(source_kind);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.disable_video(source_kind).await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_video(
    this: *mut RoomHandle,
    source_kind: i32,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let source_kind = MediaSourceKind::from(source_kind);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.enable_video(source_kind).await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_remove_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.disable_remote_audio().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_remote_audio(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.enable_remote_audio().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__disable_remote_video(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.disable_remote_video().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__enable_remote_video(
    this: *mut RoomHandle,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.enable_remote_video().await;
        completer.complete(());
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_close(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = Box::from_raw(this);
    this.on_close(DartCallback::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_connection_loss(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = Box::from_raw(this);
    this.on_connection_loss(DartCallback::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_local_track(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = Box::from_raw(this);
    this.on_local_track(DartCallback::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_new_connection(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = Box::from_raw(this);
    this.on_new_connection(DartCallback::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__free(this: *mut RoomHandle) {
    Box::from_raw(this);
}
