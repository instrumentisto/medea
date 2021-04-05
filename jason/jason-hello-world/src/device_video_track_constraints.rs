use std::convert::TryFrom;

use crate::utils::from_dart_string;

pub struct DeviceVideoTrackConstraints;

impl DeviceVideoTrackConstraints {
    pub fn device_id(&mut self, id: String) {}

    pub fn exact_facing_mode(&mut self, facing_mode: FacingMode) {}

    pub fn ideal_facing_mode(&mut self, facing_mode: FacingMode) {}

    pub fn exact_height(&mut self, height: u32) {}

    pub fn ideal_height(&mut self, height: u32) {}

    pub fn height_in_range(&mut self, min: u32, max: u32) {}

    pub fn exact_width(&mut self, width: u32) {}

    pub fn ideal_width(&mut self, width: u32) {}

    pub fn width_in_range(&mut self, min: u32, max: u32) {}
}

pub enum FacingMode {}

impl TryFrom<u8> for FacingMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        todo!()
    }
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__device_id(
    this: *mut DeviceVideoTrackConstraints,
    device_id: *const libc::c_char,
) {
    let mut this = Box::from_raw(this);
    this.device_id(from_dart_string(device_id));
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let mut this = Box::from_raw(this);
    let facing_mode = FacingMode::try_from(facing_mode).unwrap();
    this.exact_facing_mode(facing_mode);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let mut this = Box::from_raw(this);
    let facing_mode = FacingMode::try_from(facing_mode).unwrap();
    this.ideal_facing_mode(facing_mode);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let mut this = Box::from_raw(this);
    this.exact_height(height);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraits__ideal_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let mut this = Box::from_raw(this);
    this.ideal_height(height);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__height_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let mut this = Box::from_raw(this);
    this.height_in_range(min, max);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let mut this = Box::from_raw(this);
    this.exact_width(width);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraits__ideal_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let mut this = Box::from_raw(this);
    this.ideal_width(width);
}

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__width_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let mut this = Box::from_raw(this);
    this.width_in_range(min, max);
}
