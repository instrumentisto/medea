use crate::into_dart_string;

struct RoomCloseReason;

impl RoomCloseReason {
    pub fn reason(&self) -> String {
        "RoomClose reason string".to_string()
    }

    pub fn is_closed_by_server(&self) -> bool {
        false
    }

    pub fn is_err(&self) -> bool {
        false
    }
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__reason(
    this: *mut RoomCloseReason,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    into_dart_string(this.reason())
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_closed_by_server(
    this: *mut RoomCloseReason,
) -> bool {
    let this = Box::from_raw(this);
    this.is_closed_by_server()
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_err(
    this: *mut RoomCloseReason,
) -> bool {
    let this = Box::from_raw(this);
    this.is_err()
}
