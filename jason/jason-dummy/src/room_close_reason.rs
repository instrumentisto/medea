use crate::utils::{ptr_from_dart_as_mut, string_into_c_str};

pub struct RoomCloseReason;

impl RoomCloseReason {
    pub fn reason(&self) -> String {
        String::from("RoomCloseReason.reason")
    }

    pub fn is_closed_by_server(&self) -> bool {
        false
    }

    pub fn is_err(&self) -> bool {
        true
    }
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__reason(
    this: *mut RoomCloseReason,
) -> *const libc::c_char {
    let reason = ptr_from_dart_as_mut(this).reason();
    string_into_c_str(reason)
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_closed_by_server(
    this: *mut RoomCloseReason,
) -> u8 {
    ptr_from_dart_as_mut(this).is_closed_by_server() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_err(
    this: *mut RoomCloseReason,
) -> u8 {
    ptr_from_dart_as_mut(this).is_err() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__free(this: *mut RoomCloseReason) {
    Box::from_raw(this);
}
