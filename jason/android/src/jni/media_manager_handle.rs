use super::*;

use crate::{
    jni::util::JForeignObjectsArray, InputDeviceInfo, LocalMediaTrack,
    MediaManagerHandle, MediaStreamSettings,
};

impl ForeignClass for MediaManagerHandle {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeEnumerateDevices(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> JForeignObjectsArray<InputDeviceInfo> {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this = unsafe {
            MediaManagerHandle::get_ptr(this)
                .as_mut()
                .unwrap()
        };

        this.enumerate_devices().await
    });

    match result {
        Ok(devices) => env.new_object_array(devices),
        Err(msg) => {
            env.throw_new(&msg);
            JForeignObjectsArray::jni_invalid_value()
        }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeInitLocalTracks(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    caps: jlong,
) -> JForeignObjectsArray<LocalMediaTrack> {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let caps = unsafe {
            MediaStreamSettings::get_ptr(caps)
                .as_mut()
                .unwrap()
        };
        let this = unsafe {
            MediaManagerHandle::get_ptr(this)
                .as_mut()
                .unwrap()
        };

        this.init_local_tracks(caps).await
    });

    match result {
        Ok(tracks) => env.new_object_array(tracks),
        Err(msg) => {
            env.throw_new(&msg);
            JForeignObjectsArray::jni_invalid_value()
        }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        MediaManagerHandle::get_boxed(ptr);
    })
}
