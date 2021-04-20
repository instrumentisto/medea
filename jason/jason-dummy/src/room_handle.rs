use crate::{
    connection_handle::ConnectionHandle, local_media_track::LocalMediaTrack,
    reconnect_handle::ReconnectHandle, room_close_reason::RoomCloseReason,
    utils::DartCallback,
};
use dart_sys::Dart_Handle;

pub struct RoomHandle;

impl RoomHandle {
    pub fn on_new_connection(&self, cb: DartCallback<ConnectionHandle>) {
        // Result<(), JasonError>
        cb.call(ConnectionHandle);
    }

    pub fn on_close(&self, cb: DartCallback<RoomCloseReason>) {
        // Result<(), JasonError>
        cb.call(RoomCloseReason);
    }

    pub fn on_local_track(&self, cb: DartCallback<LocalMediaTrack>) {
        // Result<(), JasonError>
        cb.call(LocalMediaTrack);
    }

    pub fn on_connection_loss(&self, cb: DartCallback<ReconnectHandle>) {
        // Result<(), JasonError>
        cb.call(ReconnectHandle);
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
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_new_connection(DartCallback::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_close(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_close(DartCallback::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_local_track(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_local_track(DartCallback::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_connection_loss(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_connection_loss(DartCallback::new(cb));
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__free(this: *mut RoomHandle) {
    Box::from_raw(this);
}
