use std::ptr::NonNull;

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
    this: NonNull<RoomCloseReason>,
) -> *const libc::c_char {
    let this = this.as_ref();

    string_into_c_str(this.reason())
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_closed_by_server(
    this: NonNull<RoomCloseReason>,
) -> u8 {
    let this = this.as_ref();

    this.is_closed_by_server() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_err(
    this: NonNull<RoomCloseReason>,
) -> u8 {
    let this = this.as_ref();

    this.is_err() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__free(this: NonNull<RoomCloseReason>) {
    RoomCloseReason::from_ptr(this);
}
