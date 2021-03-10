use std::sync::Arc;

use super::*;

use crate::RemoteMediaTrack;

impl ForeignClass for RemoteMediaTrack {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeEnabled(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            RemoteMediaTrack::get_ptr(this).as_mut().unwrap()
        };
        this.enabled() as jboolean
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeOnEnabled(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: JObject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            RemoteMediaTrack::get_ptr(this).as_mut().unwrap()
        };

        this.on_enabled(Arc::new(cb))
    });
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeOnDisabled(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: JObject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            RemoteMediaTrack::get_ptr(this).as_mut().unwrap()
        };

        this.on_disabled(Arc::new(cb));
    });
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeKind(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            RemoteMediaTrack::get_ptr(this).as_mut().unwrap()
        };
        this.kind() as i32
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeMediaSourceKind(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            RemoteMediaTrack::get_ptr(this).as_mut().unwrap()
        };
        this.media_source_kind() as i32
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        RemoteMediaTrack::get_boxed(ptr);
    });
}
