use super::*;

use crate::DisplayVideoTrackConstraints;

impl ForeignClass for DisplayVideoTrackConstraints {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DisplayVideoTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        DisplayVideoTrackConstraints::get_boxed(ptr);
    })
}
