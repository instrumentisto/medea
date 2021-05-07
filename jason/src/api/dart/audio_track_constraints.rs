//! Constraints applicable to audio tracks.

use std::{os::raw::c_char, ptr::NonNull};

use super::{utils::c_str_into_string, ForeignClass};

pub use crate::media::AudioTrackConstraints;

impl ForeignClass for AudioTrackConstraints {}

/// Creates new [`AudioTrackConstraints`] with none constraints configured.
#[no_mangle]
pub extern "C" fn AudioTrackConstraints__new() -> NonNull<AudioTrackConstraints>
{
    AudioTrackConstraints::new().into_ptr()
}

/// Sets an exact [deviceId][1] constraint.
///
/// [1]: https://w3.org/TR/mediacapture-streams#def-constraint-deviceId
#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__device_id(
    mut this: NonNull<AudioTrackConstraints>,
    device_id: *const c_char,
) {
    let this = this.as_mut();

    this.device_id(c_str_into_string(device_id))
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__free(
    this: NonNull<AudioTrackConstraints>,
) {
    drop(AudioTrackConstraints::from_ptr(this));
}
