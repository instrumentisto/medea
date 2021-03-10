use std::{ptr, sync::Arc};

use jni_sys::{jclass, jlong, jstring};

use crate::{
    jni::{ForeignClass, JavaCallback},
    rust_exec_context,
    util::JNIEnv,
    ConnectionHandle,
};
use jni::objects::JObject;

impl ForeignClass for ConnectionHandle {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnClose(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: JObject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            ConnectionHandle::get_ptr(this).as_mut().unwrap()
        };
        this.on_close(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeGetRemoteMemberId<
    'a,
>(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            ConnectionHandle::get_ptr(this).as_mut().unwrap()
        };
        this.get_remote_member_id()
    });

    match result {
        Ok(remote_member_id) => {
            env.string_to_jstring(remote_member_id).into_inner()
        }
        Err(msg) => {
            env.throw_new(&msg);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnRemoteTrackAdded(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: JObject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            ConnectionHandle::get_ptr(this).as_mut().unwrap()
        };
        this.on_remote_track_added(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnQualityScoreUpdate(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: JObject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            ConnectionHandle::get_ptr(this).as_mut().unwrap()
        };
        this.on_quality_score_update(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
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
