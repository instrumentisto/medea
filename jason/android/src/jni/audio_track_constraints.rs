use jni_sys::{jclass, jfieldID, jlong, jstring, JNIEnv};

use crate::{
    jlong_to_pointer,
    jni::{
        ForeignClass, JavaString, FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS,
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD,
    },
    rust_exec_context, AudioTrackConstraints,
};

impl ForeignClass for AudioTrackConstraints {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
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

    rust_exec_context().blocking_exec(move || {
        let this = unsafe {
            jlong_to_pointer::<AudioTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.device_id(device_id);
    });
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        AudioTrackConstraints::get_boxed(ptr);
    })
}