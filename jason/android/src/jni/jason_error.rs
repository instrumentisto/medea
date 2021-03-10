use jni_sys::{jclass, jlong, jstring};

use crate::{
    jni::ForeignClass, rust_exec_context, util::JNIEnv,
    JasonError,
};

impl ForeignClass for JasonError {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeName<'a>(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let name = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { JasonError::get_ptr(this).as_mut().unwrap() };
        this.name()
    });

    env.string_to_jstring(name).into_inner()
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeMessage<'a>(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let message = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { JasonError::get_ptr(this).as_mut().unwrap() };
        this.message()
    });

    env.string_to_jstring(message).into_inner()
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeTrace<'a>(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let trace = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { JasonError::get_ptr(this).as_mut().unwrap() };
        this.trace()
    });

    env.string_to_jstring(trace).into_inner()
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        JasonError::get_boxed(ptr);
    })
}
