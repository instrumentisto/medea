use std::convert::TryFrom;

use crate::media::FacingMode;

use super::{utils::c_str_into_string, ForeignClass};

pub use crate::media::DeviceVideoTrackConstraints;

impl ForeignClass for DeviceVideoTrackConstraints {}

impl From<u8> for FacingMode {
    fn from(value: u8) -> Self {
        match value {
            0 => FacingMode::User,
            1 => FacingMode::Environment,
            2 => FacingMode::Left,
            3 => FacingMode::Right,
            _ => {
                unreachable!()
            }
        }
    }
}

/// Creates new [`DeviceVideoTrackConstraints`] with none constraints
/// configured.
#[no_mangle]
pub extern "C" fn DeviceVideoTrackConstraints__new(
) -> *const DeviceVideoTrackConstraints {
    DeviceVideoTrackConstraints::new().into_ptr()
}

/// Sets exact [deviceId][1] constraint.
///
/// [1]: https://w3.org/TR/mediacapture-streams#def-constraint-deviceId
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__device_id(
    this: *mut DeviceVideoTrackConstraints,
    device_id: *const libc::c_char,
) {
    let this = this.as_mut().unwrap();

    this.device_id(c_str_into_string(device_id));
}

/// Sets exact [facingMode][1] constraint.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-constraindomstring
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let this = this.as_mut().unwrap();

    this.exact_facing_mode(FacingMode::try_from(facing_mode).unwrap());
}

/// Sets ideal [facingMode][1] constraint.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-constraindomstring
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let this = this.as_mut().unwrap();

    this.ideal_facing_mode(FacingMode::try_from(facing_mode).unwrap());
}

/// Sets exact [`height`][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-height
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let this = this.as_mut().unwrap();

    this.exact_height(height);
}

/// Sets ideal [`height`][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-height
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let this = this.as_mut().unwrap();

    this.ideal_height(height);
}

/// Sets range of [`height`][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-height
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__height_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let this = this.as_mut().unwrap();

    this.height_in_range(min, max);
}

/// Sets exact [`width`][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-width
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let this = this.as_mut().unwrap();

    this.exact_width(width);
}

/// Sets ideal [`width`][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-width
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let this = this.as_mut().unwrap();

    this.ideal_width(width);
}

/// Sets range of [`width`][1] constraint.
///
/// [1]: https://tinyurl.com/w3-streams#def-constraint-width
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__width_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let this = this.as_mut().unwrap();

    this.width_in_range(min, max);
}

/// Frees the data behind the provided pointer.
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__free(
    this: *mut DeviceVideoTrackConstraints,
) {
    DeviceVideoTrackConstraints::from_ptr(this);
}
