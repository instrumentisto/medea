use super::*;

impl ForeignClass for RemoteMediaTrack {
    type PointedType = RemoteMediaTrack;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_REMOTEMEDIATRACK }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_REMOTEMEDIATRACK_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x = unsafe {
            jlong_to_pointer::<RemoteMediaTrack>(x).as_mut().unwrap()
        };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeEnabled(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    let this =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    this.enabled() as jboolean
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeOnEnabled(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb = <Box<dyn Consumer<()>>>::swig_from(cb, env);
    let this =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    this.on_enabled(cb)
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeOnDisabled(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb = <Box<dyn Consumer<()>>>::swig_from(cb, env);
    let this =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    this.on_disabled(cb);
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    this.kind() as i32
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeMediaSourceKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    this.media_source_kind() as i32
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    RemoteMediaTrack::get_boxed(ptr);
}
