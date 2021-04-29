use std::ptr::NonNull;

use dart_sys::Dart_Handle;

use crate::{
    connection_handle::ConnectionHandle, local_media_track::LocalMediaTrack,
    reconnect_handle::ReconnectHandle, room_close_reason::RoomCloseReason,
    utils::DartClosure, ForeignClass,
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

    // pub async fn join(&self, token: String) -> Result<(), JasonError>
    // pub fn on_failed_local_media(
    //     &self,
    //     f: Callback<JasonError>,
    // ) -> Result<(), JasonError> {
    // }
    // pub async fn set_local_media_settings(&self,
    // settings: &MediaStreamSettings, stop_first: bool, rollback_on_fail: bool)
    // -> Result<(), ConstraintsUpdateException> pub async fn
    // mute_audio(&self) -> Result<(), JasonError> pub async fn
    // unmute_audio(&self) -> Result<(), JasonError> pub async fn
    // mute_video(&self, source_kind: Option<MediaSourceKind>) -> Result<(),
    // JasonError> pub async fn unmute_video(&self, source_kind:
    // Option<MediaSourceKind>) -> Result<(), JasonError> pub async fn
    // disable_audio(&self) -> Result<(), JasonError> pub async fn
    // enable_audio(&self) -> Result<(), JasonError> pub async fn
    // disable_video(&self, source_kind: Option<MediaSourceKind>) -> Result<(),
    // JasonError> pub async fn enable_video(&self,source_kind:
    // Option<MediaSourceKind>) -> Result<(), JasonError> pub async fn
    // disable_remote_audio(&self) -> Result<(), JasonError> pub async fn
    // disable_remote_video(&self) -> Result<(), JasonError> pub async fn
    // enable_remote_audio(&self) -> Result<(), JasonError> pub async fn
    // enable_remote_video(&self) -> Result<(), JasonError>
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_new_connection(
    this: NonNull<RoomHandle>,
    cb: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_new_connection(DartClosure::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_close(
    this: NonNull<RoomHandle>,
    cb: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_close(DartClosure::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_local_track(
    this: NonNull<RoomHandle>,
    cb: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_local_track(DartClosure::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_connection_loss(
    this: NonNull<RoomHandle>,
    cb: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_connection_loss(DartClosure::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__free(this: NonNull<RoomHandle>) {
    RoomHandle::from_ptr(this);
}
