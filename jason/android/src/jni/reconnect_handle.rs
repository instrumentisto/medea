use std::ffi::CString;

use super::*;

use crate::ReconnectHandle;

impl ForeignClass for ReconnectHandle {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_RECONNECTHANDLE }
    }

    fn native_ptr_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_RECONNECTHANDLE_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ReconnectHandle_nativeReconnectWithDelay(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    delay_ms: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let delay_ms = u32::try_from(delay_ms)
            .expect("invalid jlong, in jlong => u32 conversation");
        let this = unsafe {
            jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap()
        };

        this.reconnect_with_delay(delay_ms).await
    });

    if let Err(msg) = result {
        env.throw_new(CString::new(msg).unwrap().as_ptr());
    };
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ReconnectHandle_nativeReconnectWithBackoff(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    starting_delay_ms: jlong,
    multiplier: jfloat,
    max_delay: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let starting_delay_ms = u32::try_from(starting_delay_ms)
            .expect("invalid jlong, in jlong => u32 conversation");
        let max_delay = u32::try_from(max_delay)
            .expect("invalid jlong, in jlong => u32 conversation");
        let this = unsafe {
            jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap()
        };

        this.reconnect_with_backoff(starting_delay_ms, multiplier, max_delay)
            .await
    });

    if let Err(msg) = result {
        env.throw_new(CString::new(msg).unwrap().as_ptr());
    };
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ReconnectHandle_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        ReconnectHandle::get_boxed(ptr);
    });
}
