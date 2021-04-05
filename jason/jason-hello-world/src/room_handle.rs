use dart_sys::Dart_Handle;

use crate::{
    connection_handle::ConnectionHandle, local_media_track::LocalMediaTrack,
    reconnect_handle::ReconnectHandle, room_close_reason::RoomCloseReason,
    DartCallback,
};

pub struct RoomHandle;

impl RoomHandle {
    pub fn on_close(&self, cb: DartCallback<RoomCloseReason>) {}

    pub fn on_local_track(&self, cb: DartCallback<LocalMediaTrack>) {}

    // TODO: on_failed_local_stream

    pub fn on_connection_loss(&self, cb: DartCallback<ReconnectHandle>) {}

    pub fn on_new_connection(&self, cb: DartCallback<ConnectionHandle>) {}
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
pub unsafe extern "C" fn RoomHandle__free(
    this: *mut RoomHandle
) {
    Box::from_raw(this);
}
