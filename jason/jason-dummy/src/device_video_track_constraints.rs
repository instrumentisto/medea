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

pub enum FacingMode {}

impl TryFrom<u8> for FacingMode {
    type Error = ();

    fn try_from(_value: u8) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__device_id(
    this: *mut DeviceVideoTrackConstraints,
    device_id: *const libc::c_char,
) {
    ptr_from_dart_as_mut(this).device_id(c_str_into_string(device_id));
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    ptr_from_dart_as_mut(this)
        .exact_facing_mode(FacingMode::try_from(facing_mode).unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    ptr_from_dart_as_mut(this)
        .ideal_facing_mode(FacingMode::try_from(facing_mode).unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    ptr_from_dart_as_mut(this).exact_height(height);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraits__ideal_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    ptr_from_dart_as_mut(this).ideal_height(height);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__height_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    ptr_from_dart_as_mut(this).height_in_range(min, max);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    ptr_from_dart_as_mut(this).exact_width(width);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraits__ideal_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    ptr_from_dart_as_mut(this).ideal_width(width);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__width_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    ptr_from_dart_as_mut(this).width_in_range(min, max);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__free(
    this: *mut DeviceVideoTrackConstraints,
) {
    Box::from_raw(this);
}
