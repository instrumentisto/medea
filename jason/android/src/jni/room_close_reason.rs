use super::*;

use crate::{util::JNIEnv, RoomCloseReason};

impl ForeignClass for RoomCloseReason {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_ROOMCLOSEREASON }
    }

    fn native_ptr_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_ROOMCLOSEREASON_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeReason(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let reason = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap()
        };
        this.reason()
    });

    env.string_to_jstring(reason)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeIsClosedByServer(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap()
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
            jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap()
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
