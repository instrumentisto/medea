use crate::utils::into_dart_string;

pub struct ConnectionHandle;

impl ConnectionHandle {
    pub fn get_remote_member_id(&self) -> String {
        "barfoo".to_string()
    }
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__get_remote_member_id(
    this: *mut ConnectionHandle,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    into_dart_string(this.get_remote_member_id())
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__free(this: *mut ConnectionHandle) {
    Box::from_raw(this);
}
