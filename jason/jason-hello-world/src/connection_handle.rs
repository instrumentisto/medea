use dart_sys::Dart_Handle;

pub struct ConnectionHandle;

impl ConnectionHandle {
    fn get_remote_member_id(&self) -> String {
        "foobar".to_string()
    }
}

#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__get_remote_member_id(
    this: *mut ConnectionHandle,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    super::into_dart_string(this.get_remote_member_id())
}
