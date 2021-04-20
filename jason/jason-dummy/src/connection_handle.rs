use crate::{
    remote_media_track::RemoteMediaTrack,
    utils::{string_into_c_str, DartCallback},
};
use dart_sys::Dart_Handle;

pub struct ConnectionHandle;

impl ConnectionHandle {
    pub fn get_remote_member_id(&self) -> String {
        //  Result<String, JasonError>
        String::from("ConnectionHandle.get_remote_member_id")
    }

    pub fn on_close(&self, f: DartCallback<()>) {
        // Result<(), JasonError>
        f.call_unit();
    }

    pub fn on_remote_track_added(&self, f: DartCallback<RemoteMediaTrack>) {
        // Result<(), JasonError>
        f.call(RemoteMediaTrack);
    }

    pub fn on_quality_score_update(&self, f: DartCallback<i32>) {
        // Result<(), JasonError>
        f.call_int(4);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_close(
    this: *const ConnectionHandle,
    f: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_close(DartCallback::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_remote_track_added(
    this: *const ConnectionHandle,
    f: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_remote_track_added(DartCallback::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_quality_score_update(
    this: *const ConnectionHandle,
    f: Dart_Handle,
) {
    let this = this.as_ref().unwrap();
    this.on_quality_score_update(DartCallback::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__get_remote_member_id(
    this: *const ConnectionHandle,
) -> *const libc::c_char {
    let this = this.as_ref().unwrap();

    string_into_c_str(this.get_remote_member_id())
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__free(this: *mut ConnectionHandle) {
    Box::from_raw(this);
}
