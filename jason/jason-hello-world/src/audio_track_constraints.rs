use crate::utils::from_dart_string;

pub struct AudioTrackConstraints;

impl AudioTrackConstraints {
    pub fn native_device_id(&mut self, _: String) {}
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__native_device_id(
    this: *mut AudioTrackConstraints,
    device_id: *const libc::c_char,
) {
    let mut this = Box::from_raw(this);
    // TODO: drop strings on Dart side
    this.native_device_id(from_dart_string(device_id))
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__free(
    this: *mut AudioTrackConstraints,
) {
    Box::from_raw(this);
}
