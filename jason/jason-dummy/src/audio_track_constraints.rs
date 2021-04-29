use std::ptr::NonNull;

use crate::{utils::c_str_into_string, ForeignClass};

pub struct AudioTrackConstraints;

impl ForeignClass for AudioTrackConstraints {}

impl AudioTrackConstraints {
    pub fn new() -> Self {
        Self
    }

    pub fn device_id(&mut self, _: String) {}
}

#[no_mangle]
pub extern "C" fn AudioTrackConstraints__new() -> *const AudioTrackConstraints {
    AudioTrackConstraints::new().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__device_id(
    mut this: NonNull<AudioTrackConstraints>,
    device_id: *const libc::c_char,
) {
    let this = this.as_mut();

    this.device_id(c_str_into_string(device_id))
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__free(
    this: NonNull<AudioTrackConstraints>,
) {
    AudioTrackConstraints::from_ptr(this);
}
