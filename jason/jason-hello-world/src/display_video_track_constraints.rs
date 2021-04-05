pub struct DisplayVideoTrackConstraints;

#[no_mangle]
pub unsafe extern "C" fn DeviceVideoTrackConstraints__free(
    this: *mut DisplayVideoTrackConstraints,
) {
    Box::from_raw(this);
}
