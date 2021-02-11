use super::*;

impl ForeignClass for RemoteMediaTrack {
    type PointedType = RemoteMediaTrack;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_REMOTEMEDIATRACK }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_REMOTEMEDIATRACK_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<RemoteMediaTrack> = Box::new(this);
        let this: *mut RemoteMediaTrack = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut RemoteMediaTrack = unsafe {
            jlong_to_pointer::<RemoteMediaTrack>(x).as_mut().unwrap()
        };
        let x: Box<RemoteMediaTrack> = unsafe { Box::from_raw(x) };
        let x: RemoteMediaTrack = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut RemoteMediaTrack = unsafe {
            jlong_to_pointer::<RemoteMediaTrack>(x).as_mut().unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeEnabled(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: bool = RemoteMediaTrack::enabled(this);
    ret as jboolean
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeOnEnabled(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    callback: jobject,
) {
    let callback: Box<dyn Callback> =
        <Box<dyn Callback>>::swig_from(callback, env);
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: () = RemoteMediaTrack::on_enabled(this, callback);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeOnDisabled(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    callback: jobject,
) {
    let callback: Box<dyn Callback> =
        <Box<dyn Callback>>::swig_from(callback, env);
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: () = RemoteMediaTrack::on_disabled(this, callback);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: MediaKind = RemoteMediaTrack::kind(this);
    let ret: jint = ret.as_jint();
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeMediaSourceKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: MediaSourceKind = RemoteMediaTrack::media_source_kind(this);
    let ret: jint = ret.as_jint();
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let this: Box<RemoteMediaTrack> = unsafe { Box::from_raw(this) };
    drop(this);
}
