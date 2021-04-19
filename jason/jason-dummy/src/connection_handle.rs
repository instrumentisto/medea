use crate::utils::{ptr_from_dart_as_ref, string_into_c_str};

pub struct ConnectionHandle;

impl ConnectionHandle {
    pub fn get_remote_member_id(&self) -> String {
        //  Result<String, JasonError>
        String::from("ConnectionHandle.get_remote_member_id")
    }

    // pub fn on_close(&self, f: Callback<()>) -> Result<(), JasonError> { }
    // pub fn on_remote_track_added(&self, f: Callback<RemoteMediaTrack>) ->
    // Result<(), JasonError> { } pub fn on_quality_score_update(&self, f:
    // Callback<u8>) -> Result<(), JasonError> {}
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__get_remote_member_id(
    this: *const ConnectionHandle,
) -> *const libc::c_char {
    let this = ptr_from_dart_as_ref(this);

    string_into_c_str(this.get_remote_member_id())
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__free(this: *mut ConnectionHandle) {
    if !this.is_null() {
        Box::from_raw(this);
    }
}
