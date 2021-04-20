use crate::utils::c_str_into_string;

pub struct AudioTrackConstraints;

impl AudioTrackConstraints {
    pub fn new() -> Self {
        Self
    }

    pub fn device_id(&mut self, _: String) {}
}

#[no_mangle]
pub extern "C" fn AudioTrackConstraints__new() -> *const AudioTrackConstraints {
    Box::into_raw(Box::new(AudioTrackConstraints::new()))
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__device_id(
    this: *mut AudioTrackConstraints,
    device_id: *const libc::c_char,
) {
    let this = this.as_mut().unwrap();

    this.device_id(c_str_into_string(device_id))
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__free(
    this: *mut AudioTrackConstraints,
) {
    Box::from_raw(this);
}
