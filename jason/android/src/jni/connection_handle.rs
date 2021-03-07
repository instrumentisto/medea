use std::ptr;

use jni_sys::{jclass, jfieldID, jlong, jobject, jstring};

use crate::{
    jlong_to_pointer,
    jni::{
        ForeignClass, JavaCallback, FOREIGN_CLASS_CONNECTIONHANDLE,
        FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD,
    },
    rust_exec_context,
    util::JNIEnv,
    ConnectionHandle,
};
use std::{ffi::CString, sync::Arc};

impl ForeignClass for ConnectionHandle {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_CONNECTIONHANDLE }
    }

    fn native_ptr_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnClose(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap()
        };
        this.on_close(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(CString::new(msg).unwrap().as_ptr());
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeGetRemoteMemberId(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap()
        };
        this.get_remote_member_id()
    });

    match result {
        Ok(remote_member_id) => env.string_to_jstring(remote_member_id),
        Err(msg) => {
            env.throw_new(CString::new(msg).unwrap().as_ptr());
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnRemoteTrackAdded(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap()
        };
        this.on_remote_track_added(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(CString::new(msg).unwrap().as_ptr());
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnQualityScoreUpdate(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap()
        };
        this.on_quality_score_update(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(CString::new(msg).unwrap().as_ptr());
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        ConnectionHandle::get_boxed(ptr);
    })
}
