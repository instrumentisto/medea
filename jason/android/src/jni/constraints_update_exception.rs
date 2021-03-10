use jni_sys::{jclass, jlong, jstring};

use crate::{
    jni::{rust_exec_context, ForeignClass},
    util::{JForeignObjectsArray, JNIEnv},
    ConstraintsUpdateException, JasonError,
};

impl ForeignClass for ConstraintsUpdateException {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeName<
    'a,
>(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let name = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            ConstraintsUpdateException::get_ptr(this).as_mut().unwrap()
        };

        this.name()
    });

    env.string_to_jstring(name).into_inner()
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeRecoverReason(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let recover_reason = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            ConstraintsUpdateException::get_ptr(this).as_mut().unwrap()
        };

        this.recover_reason()
    });
    match recover_reason {
        None => {
            // TODO: make sure that this is null in java
            0
        }
        Some(recover_reason) => recover_reason.box_object(),
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeRecoverFailReason(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> JForeignObjectsArray<JasonError> {
    let env = unsafe { JNIEnv::from_raw(env) };
    let recover_fail_reasons = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            ConstraintsUpdateException::get_ptr(this).as_mut().unwrap()
        };

        this.recover_fail_reasons()
    });

    env.new_object_array(recover_fail_reasons)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeError(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let error = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            ConstraintsUpdateException::get_ptr(this).as_mut().unwrap()
        };

        this.error()
    });

    match error {
        Some(jason_error) => jason_error.box_object(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        ConstraintsUpdateException::get_boxed(ptr);
    })
}
