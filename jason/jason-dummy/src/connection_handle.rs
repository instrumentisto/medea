use std::ptr::NonNull;

use dart_sys::Dart_Handle;

use crate::{
    remote_media_track::RemoteMediaTrack,
    utils::{string_into_c_str, DartClosure},
    ForeignClass,
};

pub struct ConnectionHandle;

impl ForeignClass for ConnectionHandle {}

impl ConnectionHandle {
    pub fn get_remote_member_id(&self) -> String {
        //  Result<String, JasonError>
        String::from("ConnectionHandle.get_remote_member_id")
    }

    pub fn on_close(&self, f: DartClosure<()>) {
        // Result<(), JasonError>
        f.call0();
    }

    pub fn on_remote_track_added(&self, f: DartClosure<RemoteMediaTrack>) {
        // Result<(), JasonError>
        f.call1(RemoteMediaTrack);
    }

    pub fn on_quality_score_update(&self, f: DartClosure<u8>) {
        // Result<(), JasonError>
        f.call1(4);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_close(
    this: NonNull<ConnectionHandle>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_close(DartClosure::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_remote_track_added(
    this: NonNull<ConnectionHandle>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_remote_track_added(DartClosure::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_quality_score_update(
    this: NonNull<ConnectionHandle>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_quality_score_update(DartClosure::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__get_remote_member_id(
    this: NonNull<ConnectionHandle>,
) -> *const libc::c_char {
    let this = this.as_ref();

    string_into_c_str(this.get_remote_member_id())
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__free(
    this: NonNull<ConnectionHandle>,
) {
    ConnectionHandle::from_ptr(this);
}
