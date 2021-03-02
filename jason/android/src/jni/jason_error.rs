use jni_sys::{jclass, jfieldID, jlong, jstring};

use crate::{
    jlong_to_pointer,
    jni::{
        ForeignClass, FOREIGN_CLASS_JASONERROR,
        FOREIGN_CLASS_JASONERROR_NATIVEPTR_FIELD,
    },
    rust_exec_context,
    util::JNIEnv,
    JasonError,
};

impl ForeignClass for JasonError {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_JASONERROR }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_JASONERROR_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeName(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let name = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
        this.name()
    });

    env.string_to_jstring(name)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeMessage(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let message = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
        this.message()
    });

    env.string_to_jstring(message)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeTrace(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let env = unsafe { JNIEnv::from_raw(env) };
    let trace = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
        this.trace()
    });

    env.string_to_jstring(trace)
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
