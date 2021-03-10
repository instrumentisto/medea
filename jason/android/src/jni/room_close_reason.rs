use super::*;

use crate::{util::JNIEnv, RoomCloseReason};

impl ForeignClass for RoomCloseReason {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeReason<'a>(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let reason = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            RoomCloseReason::get_ptr(this).as_mut().unwrap()
        };
        this.reason()
    });

    env.string_to_jstring(reason).into_inner()
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeIsClosedByServer(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            RoomCloseReason::get_ptr(this).as_mut().unwrap()
        };
        this.is_closed_by_server()
    }) as jboolean
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeRoomCloseReason(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    rust_exec_context().blocking_exec(move || {
        let this: &RoomCloseReason = unsafe {
            RoomCloseReason::get_ptr(this).as_mut().unwrap()
        };
        let ret: bool = RoomCloseReason::is_err(this);
        ret as jboolean
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        RoomCloseReason::get_boxed(ptr);
    });
}
