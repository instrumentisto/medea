use super::*;

impl ForeignClass for MediaManagerHandle {
    type PointedType = MediaManagerHandle;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_MEDIAMANAGERHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_MEDIAMANAGERHANDLE_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<MediaManagerHandle> = Box::new(this);
        let this: *mut MediaManagerHandle = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut MediaManagerHandle = unsafe {
            jlong_to_pointer::<MediaManagerHandle>(x).as_mut().unwrap()
        };
        let x: Box<MediaManagerHandle> = unsafe { Box::from_raw(x) };
        let x: MediaManagerHandle = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut MediaManagerHandle = unsafe {
            jlong_to_pointer::<MediaManagerHandle>(x).as_mut().unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeEnumerateDevices(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> JForeignObjectsArray<InputDeviceInfo> {
    let this: &MediaManagerHandle = unsafe {
        jlong_to_pointer::<MediaManagerHandle>(this)
            .as_mut()
            .unwrap()
    };
    let ret: Result<Vec<InputDeviceInfo>, String> =
        MediaManagerHandle::enumerate_devices(this);
    let ret: JForeignObjectsArray<InputDeviceInfo> = match ret {
        Ok(x) => {
            let ret: JForeignObjectsArray<InputDeviceInfo> =
                vec_of_objects_to_jobject_array(env, x);
            ret
        }
        Err(msg) => {
            jni_throw_exception(env, &msg);
            return <JForeignObjectsArray<InputDeviceInfo>>::jni_invalid_value(
            );
        }
    };
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeInitLocalTracks(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    caps: jlong,
) -> JForeignObjectsArray<LocalMediaTrack> {
    let caps: &MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(caps)
            .as_mut()
            .unwrap()
    };
    let this: &MediaManagerHandle = unsafe {
        jlong_to_pointer::<MediaManagerHandle>(this)
            .as_mut()
            .unwrap()
    };
    let ret: Result<Vec<LocalMediaTrack>, String> =
        MediaManagerHandle::init_local_tracks(this, caps);
    let ret: JForeignObjectsArray<LocalMediaTrack> = match ret {
        Ok(x) => {
            let ret: JForeignObjectsArray<LocalMediaTrack> =
                vec_of_objects_to_jobject_array(env, x);
            ret
        }
        Err(msg) => {
            jni_throw_exception(env, &msg);
            return <JForeignObjectsArray<LocalMediaTrack>>::jni_invalid_value(
            );
        }
    };
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut MediaManagerHandle = unsafe {
        jlong_to_pointer::<MediaManagerHandle>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<MediaManagerHandle> = unsafe { Box::from_raw(this) };
    drop(this);
}
