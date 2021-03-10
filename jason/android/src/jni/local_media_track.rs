use super::*;

use crate::LocalMediaTrack;

impl ForeignClass for LocalMediaTrack {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            LocalMediaTrack::get_ptr(this).as_mut().unwrap()
        };
        this.kind().as_jint()
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeMediaSourceKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            LocalMediaTrack::get_ptr(this).as_mut().unwrap()
        };
        this.media_source_kind().as_jint()
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        LocalMediaTrack::get_boxed(ptr);
    })
}
