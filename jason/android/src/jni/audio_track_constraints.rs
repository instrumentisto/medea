use std::ptr;

use jni_sys::{jclass, jfieldID, jlong, jstring, JNIEnv};

use crate::{
    jni::{
        jlong_to_pointer, ForeignClass, JavaString,
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS,
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD,
    },
    AudioTrackConstraints,
};

impl ForeignClass for AudioTrackConstraints {
    type PointedType = AudioTrackConstraints;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(ptr: jlong) -> ptr::NonNull<Self::PointedType> {
        let this = unsafe {
            jlong_to_pointer::<AudioTrackConstraints>(ptr)
                .as_mut()
                .unwrap()
        };
        ptr::NonNull::<Self::PointedType>::new(this).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeDeviceId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    device_id: jstring,
) {
    let device_id = JavaString::new(env, device_id).to_str().to_owned();
    let this = unsafe {
        jlong_to_pointer::<AudioTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    this.device_id(device_id);
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    AudioTrackConstraints::get_boxed(ptr);
}
