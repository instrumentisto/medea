use std::{convert::TryFrom, ptr::NonNull};

use libc::c_char;

use crate::media::FacingMode;

use super::{utils::c_str_into_string, ForeignClass};

pub use crate::media::DeviceVideoTrackConstraints;

impl ForeignClass for DeviceVideoTrackConstraints {}

/// Creates new [`DeviceVideoTrackConstraints`] with none constraints
/// configured.
#[no_mangle]
pub extern "C" fn DeviceVideoTrackConstraints__new(
) -> NonNull<DeviceVideoTrackConstraints> {
    DeviceVideoTrackConstraints::new().into_ptr()
}

/// Sets an exact [deviceId][1] constraint.
///
/// [1]: https://w3.org/TR/mediacapture-streams#def-constraint-deviceId
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__device_id(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    device_id: NonNull<c_char>,
) {
    let this = this.as_mut();

    this.device_id(c_str_into_string(device_id));
}

/// Sets an exact [facingMode][1] constraint.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-constraindomstring
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_facing_mode(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    facing_mode: FacingMode,
) {
    let this = this.as_mut();

    this.exact_facing_mode(facing_mode);
}

/// Sets an ideal [facingMode][1] constraint.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-constraindomstring
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_facing_mode(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    facing_mode: FacingMode,
) {
    let this = this.as_mut();

    this.ideal_facing_mode(facing_mode);
}

/// Sets an exact [height][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-height
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_height(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    height: u32,
) {
    let this = this.as_mut();

    this.exact_height(height);
}

/// Sets an ideal [height][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-height
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_height(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    height: u32,
) {
    let this = this.as_mut();

    this.ideal_height(height);
}

/// Sets a range of a [height][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-height
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__height_in_range(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    min: u32,
    max: u32,
) {
    let this = this.as_mut();

    this.height_in_range(min, max);
}

/// Sets an exact [width][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-width
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_width(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    width: u32,
) {
    let this = this.as_mut();

    this.exact_width(width);
}

/// Sets an ideal [width][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-width
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_width(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    width: u32,
) {
    let this = this.as_mut();

    this.ideal_width(width);
}

/// Sets a range of a [width][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-width
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__width_in_range(
    mut this: NonNull<DeviceVideoTrackConstraints>,
    min: u32,
    max: u32,
) {
    let this = this.as_mut();

    this.width_in_range(min, max);
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__free(
    this: NonNull<DeviceVideoTrackConstraints>,
) {
    drop(DeviceVideoTrackConstraints::from_ptr(this));
}
