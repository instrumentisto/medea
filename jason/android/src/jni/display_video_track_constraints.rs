use super::*;

impl ForeignClass for DisplayVideoTrackConstraints {
    type PointedType = DisplayVideoTrackConstraints;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<DisplayVideoTrackConstraints> = Box::new(this);
        let this: *mut DisplayVideoTrackConstraints = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut DisplayVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DisplayVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        let x: Box<DisplayVideoTrackConstraints> = unsafe { Box::from_raw(x) };
        let x: DisplayVideoTrackConstraints = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut DisplayVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DisplayVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DisplayVideoTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut DisplayVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DisplayVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<DisplayVideoTrackConstraints> =
        unsafe { Box::from_raw(this) };
    drop(this);
}
