use super::*;

impl ForeignClass for LocalMediaTrack {
    type PointedType = LocalMediaTrack;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_LOCALMEDIATRACK }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_LOCALMEDIATRACK_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x: *mut LocalMediaTrack =
            unsafe { jlong_to_pointer::<LocalMediaTrack>(x).as_mut().unwrap() };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &LocalMediaTrack =
        unsafe { jlong_to_pointer::<LocalMediaTrack>(this).as_mut().unwrap() };
    let ret: MediaKind = LocalMediaTrack::kind(this);
    let ret: jint = ret.as_jint();
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeMediaSourceKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &LocalMediaTrack =
        unsafe { jlong_to_pointer::<LocalMediaTrack>(this).as_mut().unwrap() };
    let ret: MediaSourceKind = LocalMediaTrack::media_source_kind(this);
    let ret: jint = ret.as_jint();
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    LocalMediaTrack::get_boxed(ptr);
}
