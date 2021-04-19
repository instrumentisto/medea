use crate::utils::{ptr_from_dart_as_ref, string_into_c_str};

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
    this: *const RoomCloseReason,
) -> *const libc::c_char {
    let this = ptr_from_dart_as_ref(this);

    string_into_c_str(this.reason())
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_closed_by_server(
    this: *const RoomCloseReason,
) -> u8 {
    let this = ptr_from_dart_as_ref(this);

    this.is_closed_by_server() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_err(
    this: *const RoomCloseReason,
) -> u8 {
    let this = ptr_from_dart_as_ref(this);

    this.is_err() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__free(this: *mut RoomCloseReason) {
    if !this.is_null() {
        Box::from_raw(this);
    }
}
