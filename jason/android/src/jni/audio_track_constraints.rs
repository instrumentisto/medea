use super::*;

impl ForeignClass for AudioTrackConstraints {
    type PointedType = AudioTrackConstraints;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<AudioTrackConstraints> = Box::new(this);
        let this: *mut AudioTrackConstraints = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut AudioTrackConstraints = unsafe {
            jlong_to_pointer::<AudioTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        let x: Box<AudioTrackConstraints> = unsafe { Box::from_raw(x) };
        let x: AudioTrackConstraints = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut AudioTrackConstraints = unsafe {
            jlong_to_pointer::<AudioTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeDeviceId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    device_id: jstring,
) {
    let device_id: JavaString = JavaString::new(env, device_id);
    let device_id: &str = device_id.to_str();
    let device_id: String = device_id.to_string();
    let this: &mut AudioTrackConstraints = unsafe {
        jlong_to_pointer::<AudioTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = AudioTrackConstraints::device_id(this, device_id);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut AudioTrackConstraints = unsafe {
        jlong_to_pointer::<AudioTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<AudioTrackConstraints> = unsafe { Box::from_raw(this) };
    drop(this);
}
