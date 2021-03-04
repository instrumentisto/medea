use super::*;

use crate::{util::JNIEnv, InputDeviceInfo};

impl ForeignClass for InputDeviceInfo {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_INPUTDEVICEINFO }
    }

    fn native_ptr_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_INPUTDEVICEINFO_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeDeviceId(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let device_id = rust_exec_context().blocking_exec(move || {
        let this: &InputDeviceInfo = unsafe {
            jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap()
        };
        this.device_id()
    });

    env.string_to_jstring(device_id)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeKind(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap()
        };
        this.kind().as_jint()
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeLabel(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let label = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap()
        };
        this.label()
    });

    env.string_to_jstring(label)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeGroupId(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let group_id = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap()
        };
        this.group_id()
    });

    env.string_to_jstring(group_id)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        InputDeviceInfo::get_boxed(ptr);
    })
}
