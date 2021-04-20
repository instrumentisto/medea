pub struct DisplayVideoTrackConstraints;

impl DisplayVideoTrackConstraints {
    fn new() -> Self {
        Self
    }
}

#[no_mangle]
pub extern "C" fn DisplayVideoTrackConstraints__new(
) -> *const DisplayVideoTrackConstraints {
    Box::into_raw(Box::new(DisplayVideoTrackConstraints::new()))
}

#[no_mangle]
pub unsafe extern "C" fn DisplayVideoTrackConstraints__free(
    this: *mut DisplayVideoTrackConstraints,
) {
    Box::from_raw(this);
}
