use super::*;

use crate::DisplayVideoTrackConstraints;

impl ForeignClass for DisplayVideoTrackConstraints {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS }
    }

    fn native_ptr_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }
}

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
