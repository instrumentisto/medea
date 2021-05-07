use std::ptr::NonNull;

use super::ForeignClass;

pub use crate::media::DisplayVideoTrackConstraints;

impl ForeignClass for DisplayVideoTrackConstraints {}

/// Creates new [`DisplayVideoTrackConstraints`] with none constraints
/// configured.
#[no_mangle]
pub extern "C" fn DisplayVideoTrackConstraints__new(
) -> NonNull<DisplayVideoTrackConstraints> {
    DisplayVideoTrackConstraints::new().into_ptr()
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn DisplayVideoTrackConstraints__free(
    this: NonNull<DisplayVideoTrackConstraints>,
) {
    let _ = DisplayVideoTrackConstraints::from_ptr(this);
}
