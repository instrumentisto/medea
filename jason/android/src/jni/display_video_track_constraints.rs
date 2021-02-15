use super::*;

impl ForeignClass for DisplayVideoTrackConstraints {
    type PointedType = DisplayVideoTrackConstraints;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x: *mut DisplayVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DisplayVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DisplayVideoTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    DisplayVideoTrackConstraints::get_boxed(ptr);
}
