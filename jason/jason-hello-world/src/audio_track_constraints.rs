use dart_sys::Dart_Handle;

pub struct AudioTrackConstraints;

impl AudioTrackConstraints {
    pub fn native_device_id(&mut self, id: String) {
    }
}

#[no_mangle]
pub unsafe extern "C" fn AudioTrackConstraints__native_device_id(
    this: *mut AudioTrackConstraints,
    device_id: *const libc::c_char,
) {
    let mut this = Box::from_raw(this);
    // TODO: drop strings on Dart side
    this.native_device_id(unsafe { super::dart_string(device_id) })
}
