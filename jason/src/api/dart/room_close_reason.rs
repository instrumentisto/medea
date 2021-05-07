use std::{os::raw::c_char, ptr::NonNull};

use super::{utils::string_into_c_str, ForeignClass};

pub use crate::room::RoomCloseReason;

impl ForeignClass for RoomCloseReason {}

/// Returns a close reason of a [`Room`].
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__reason(
    this: NonNull<RoomCloseReason>,
) -> NonNull<c_char> {
    let this = this.as_ref();

    string_into_c_str(this.reason())
}

/// Indicates whether a [`Room`] was closed by server.
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_closed_by_server(
    this: NonNull<RoomCloseReason>,
) -> u8 {
    let this = this.as_ref();

    this.is_closed_by_server() as u8
}

/// Indicates whether a [`Room`]'s close reason is considered as an error.
///
/// [`Room`]: crate::room::Room
#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__is_err(
    this: NonNull<RoomCloseReason>,
) -> u8 {
    let this = this.as_ref();

    this.is_err() as u8
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn RoomCloseReason__free(this: NonNull<RoomCloseReason>) {
    drop(RoomCloseReason::from_ptr(this));
}
