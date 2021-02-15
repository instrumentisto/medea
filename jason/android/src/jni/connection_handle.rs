use std::ptr;

use jni_sys::{jclass, jfieldID, jlong, jobject, jstring, JNIEnv};

use crate::{
    jlong_to_pointer,
    jni::{
        from_std_string_jstring, jni_throw_exception, ForeignClass, SwigFrom,
        FOREIGN_CLASS_CONNECTIONHANDLE,
        FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD,
    },
    ConnectionHandle, Consumer, RemoteMediaTrack,
};

impl ForeignClass for ConnectionHandle {
    type PointedType = ConnectionHandle;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_CONNECTIONHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(ptr: jlong) -> ptr::NonNull<Self::PointedType> {
        let this = unsafe {
            jlong_to_pointer::<ConnectionHandle>(ptr).as_mut().unwrap()
        };
        ptr::NonNull::<Self::PointedType>::new(this).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnClose(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb = <Box<dyn Consumer<()>>>::swig_from(cb, env);
    let this =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.on_close(cb) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeGetRemoteMemberId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    match this.get_remote_member_id() {
        Ok(x) => from_std_string_jstring(x, env),
        Err(msg) => {
            jni_throw_exception(env, &msg);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnRemoteTrackAdded(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    f: jobject,
) {
    let f: Box<dyn Consumer<RemoteMediaTrack>> =
        <Box<dyn Consumer<RemoteMediaTrack>>>::swig_from(f, env);
    let this: &ConnectionHandle =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    if let Err(msg) = ConnectionHandle::on_remote_track_added(this, f) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnQualityScoreUpdate(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb = <Box<dyn Consumer<u8>>>::swig_from(cb, env);
    let this =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.on_quality_score_update(cb) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    ConnectionHandle::get_boxed(ptr);
}
