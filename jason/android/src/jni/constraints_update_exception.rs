use jni_sys::{jclass, jfieldID, jlong, jstring};

use crate::{
    jni::{
        jlong_to_pointer, rust_exec_context, ForeignClass,
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION,
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_NATIVEPTR_FIELD,
    },
    util::{JForeignObjectsArray, JNIEnv},
    ConstraintsUpdateException, JasonError,
};

impl ForeignClass for ConstraintsUpdateException {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeName(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let name = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<ConstraintsUpdateException>(this)
                .as_mut()
                .unwrap()
        };

        this.name()
    });

    env.string_to_jstring(name)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeRecoverReason(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let recover_reason = rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<ConstraintsUpdateException>(this)
                .as_mut()
                .unwrap()
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
            jlong_to_pointer::<ConstraintsUpdateException>(this)
                .as_mut()
                .unwrap()
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
            jlong_to_pointer::<ConstraintsUpdateException>(this)
                .as_mut()
                .unwrap()
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
