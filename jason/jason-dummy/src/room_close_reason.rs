use crate::{utils::string_into_c_str, ForeignClass};

pub struct RoomCloseReason;

impl ForeignClass for RoomCloseReason {}

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
    let this = this.as_ref().unwrap();

    string_into_c_str(this.reason())
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_closed_by_server(
    this: *const RoomCloseReason,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.is_closed_by_server() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_err(
    this: *const RoomCloseReason,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.is_err() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__free(this: *mut RoomCloseReason) {
    RoomCloseReason::from_ptr(this);
}
