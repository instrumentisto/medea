use super::*;

use crate::{
    jni::util::JForeignObjectsArray, InputDeviceInfo, LocalMediaTrack,
    MediaManagerHandle, MediaStreamSettings,
};

impl ForeignClass for MediaManagerHandle {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_MEDIAMANAGERHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_MEDIAMANAGERHANDLE_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeEnumerateDevices(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) -> JForeignObjectsArray<InputDeviceInfo> {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this = unsafe {
            jlong_to_pointer::<MediaManagerHandle>(this)
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
            jlong_to_pointer::<MediaStreamSettings>(caps)
                .as_mut()
                .unwrap()
        };
        let this = unsafe {
            jlong_to_pointer::<MediaManagerHandle>(this)
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
