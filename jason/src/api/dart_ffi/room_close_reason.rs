use super::{utils::string_into_c_str, ForeignClass};

pub use crate::room::RoomCloseReason;

impl ForeignClass for RoomCloseReason {}

/// Returns a close reason of the [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__reason(
    this: *const RoomCloseReason,
) -> *const libc::c_char {
    let this = this.as_ref().unwrap();

    string_into_c_str(this.reason())
}

/// Indicates whether the [`Room`] was closed by server.
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_closed_by_server(
    this: *const RoomCloseReason,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.is_closed_by_server() as u8
}

/// Indicates whether the [`Room`]'s close reason is considered as an error.
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_err(
    this: *const RoomCloseReason,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.is_err() as u8
}

/// Frees the data behind the provided pointer.
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__free(this: *mut RoomCloseReason) {
    RoomCloseReason::from_ptr(this);
}
