use std::convert::TryFrom;

pub struct DeviceVideoTrackConstraints;

impl DeviceVideoTrackConstraints {
    fn device_id(&mut self, id: String) {

    }

    fn exact_facing_mode(&mut self, facing_mode: FacingMode) {

    }

    fn ideal_facing_mode(&mut self, facing_mode: FacingMode) {

    }

    fn exact_height(&mut self, height: u32) {

    }

    fn ideal_height(&mut self, height: u32) {

    }

    fn height_in_range(&mut self, min: u32, max: u32) {

    }

    fn exact_width(&mut self, width: u32) {

    }

    fn ideal_width(&mut self, width: u32) {

    }

    fn width_in_range(&mut self, min: u32, max: u32) {

    }
}

enum FacingMode {

}

impl TryFrom<u8> for FacingMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        todo!()
    }
}

pub unsafe extern "C" fn DeviceVideoTrackConstraints__device_id(
    this: *mut DeviceVideoTrackConstraints,
    device_id: *const libc::c_char,
) {
    let mut this = Box::from_raw(this);
    this.device_id(super::dart_string(device_id));
}

pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let mut this = Box::from_raw(this);
    let facing_mode = FacingMode::try_from(facing_mode).unwrap();
    this.exact_facing_mode(facing_mode);
}

pub unsafe extern "C" fn DeviceVideoTrackConstraints__ideal_facing_mode(
    this: *mut DeviceVideoTrackConstraints,
    facing_mode: u8,
) {
    let mut this = Box::from_raw(this);
    let facing_mode = FacingMode::try_from(facing_mode).unwrap();
    this.ideal_facing_mode(facing_mode);
}

pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let mut this = Box::from_raw(this);
    this.exact_height(height);
}

pub unsafe extern "C" fn DeviceVideoTrackConstraits__ideal_height(
    this: *mut DeviceVideoTrackConstraints,
    height: u32,
) {
    let mut this = Box::from_raw(this);
    this.ideal_height(height);
}

pub unsafe extern "C" fn DeviceVideoTrackConstraints__height_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let mut this = Box::from_raw(this);
    this.height_in_range(min, max);
}

pub unsafe extern "C" fn DeviceVideoTrackConstraints__exact_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let mut this = Box::from_raw(this);
    this.exact_width(width);
}

pub unsafe extern "C" fn DeviceVideoTrackConstraits__ideal_width(
    this: *mut DeviceVideoTrackConstraints,
    width: u32,
) {
    let mut this = Box::from_raw(this);
    this.ideal_width(width);
}

pub unsafe extern "C" fn DeviceVideoTrackConstraints__width_in_range(
    this: *mut DeviceVideoTrackConstraints,
    min: u32,
    max: u32,
) {
    let mut this = Box::from_raw(this);
    this.width_in_range(min, max);
}
