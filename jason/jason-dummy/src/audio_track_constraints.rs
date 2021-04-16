use crate::utils::{c_str_into_string, ptr_from_dart_as_mut};

pub struct AudioTrackConstraints;

impl AudioTrackConstraints {
    pub fn new() -> Self {
        Self
    }

    pub fn device_id(&mut self, _: String) {}
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__device_id(
    this: *mut AudioTrackConstraints,
    device_id: *const libc::c_char,
) {
    ptr_from_dart_as_mut(this).device_id(c_str_into_string(device_id))
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__free(
    this: *mut AudioTrackConstraints,
) {
    Box::from_raw(this);
}
