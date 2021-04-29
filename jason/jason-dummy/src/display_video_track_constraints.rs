use crate::ForeignClass;

pub struct DisplayVideoTrackConstraints;

impl ForeignClass for DisplayVideoTrackConstraints {}

impl DisplayVideoTrackConstraints {
    fn new() -> Self {
        Self
    }
}

#[no_mangle]
pub extern "C" fn DisplayVideoTrackConstraints__new(
) -> *const DisplayVideoTrackConstraints {
    DisplayVideoTrackConstraints::new().into_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn DisplayVideoTrackConstraints__free(
    this: *mut DisplayVideoTrackConstraints,
) {
    DisplayVideoTrackConstraints::from_ptr(this);
}
