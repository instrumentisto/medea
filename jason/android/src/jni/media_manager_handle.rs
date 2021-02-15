use super::*;

impl ForeignClass for MediaManagerHandle {
    type PointedType = MediaManagerHandle;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_MEDIAMANAGERHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_MEDIAMANAGERHANDLE_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x: *mut MediaManagerHandle = unsafe {
            jlong_to_pointer::<MediaManagerHandle>(x).as_mut().unwrap()
        };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeEnumerateDevices(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> JForeignObjectsArray<InputDeviceInfo> {
    let this = unsafe {
        jlong_to_pointer::<MediaManagerHandle>(this)
            .as_mut()
            .unwrap()
    };
    match this.enumerate_devices() {
        Ok(x) => JForeignObjectsArray::from_jobjects(env, x),
        Err(msg) => {
            jni_throw_exception(env, &msg);
            JForeignObjectsArray::jni_invalid_value()
        }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeInitLocalTracks(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    caps: jlong,
) -> JForeignObjectsArray<LocalMediaTrack> {
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
    match this.init_local_tracks(caps) {
        Ok(x) => JForeignObjectsArray::from_jobjects(env, x),
        Err(msg) => {
            jni_throw_exception(env, &msg);
            JForeignObjectsArray::jni_invalid_value()
        }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    MediaManagerHandle::get_boxed(ptr);
}
