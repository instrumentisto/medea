use std::convert::TryFrom;

use crate::utils::{c_str_into_string, ptr_from_dart_as_mut};

pub struct DeviceVideoTrackConstraints;

impl DeviceVideoTrackConstraints {
    pub fn new() -> Self {
        Self
    }

    pub fn device_id(&mut self, _device_id: String) {}

    pub fn exact_facing_mode(&mut self, _facing_mode: FacingMode) {}

    pub fn ideal_facing_mode(&mut self, _facing_mode: FacingMode) {}

    pub fn exact_height(&mut self, _height: u32) {}

    pub fn ideal_height(&mut self, _height: u32) {}

    pub fn height_in_range(&mut self, _min: u32, _max: u32) {}

    pub fn exact_width(&mut self, _width: u32) {}

    pub fn ideal_width(&mut self, _width: u32) {}

    pub fn width_in_range(&mut self, _min: u32, _max: u32) {}
}

pub enum FacingMode {
    User,
    Environment,
    Left,
    Right,
}

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

#[no_mangle]
pub extern "C" fn DeviceVideoTrackConstraints__new(
) -> *const DeviceVideoTrackConstraints {
    Box::into_raw(Box::new(DeviceVideoTrackConstraints::new()))
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__device_id(
    this: *mut DeviceVideoTrackConstraints,
    device_id: *const libc::c_char,
) {
    let this = ptr_from_dart_as_mut(this);

    this.device_id(c_str_into_string(device_id));
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let this = ptr_from_dart_as_mut(this);

    this.exact_facing_mode(FacingMode::try_from(facing_mode).unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let this = ptr_from_dart_as_mut(this);

    this.ideal_facing_mode(FacingMode::try_from(facing_mode).unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let this = ptr_from_dart_as_mut(this);

    this.exact_height(height);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let this = ptr_from_dart_as_mut(this);

    this.ideal_height(height);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__height_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let this = ptr_from_dart_as_mut(this);

    this.height_in_range(min, max);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let this = ptr_from_dart_as_mut(this);

    this.exact_width(width);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let this = ptr_from_dart_as_mut(this);

    this.ideal_width(width);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__width_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let this = ptr_from_dart_as_mut(this);

    this.width_in_range(min, max);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__free(
    this: *mut DeviceVideoTrackConstraints,
) {
    if !this.is_null() {
        Box::from_raw(this);
    }
}
