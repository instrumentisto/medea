use jni_sys::{jclass, jfieldID, jlong, jstring};

use crate::{
    jlong_to_pointer,
    jni::{
        util::JNIEnv, ForeignClass, FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS,
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD,
    },
    rust_exec_context, AudioTrackConstraints,
};

impl ForeignClass for AudioTrackConstraints {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS }
    }

    fn native_ptr_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeDeviceId(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    device_id: jstring,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let device_id = env.clone_jstring_to_string(device_id);

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
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        AudioTrackConstraints::get_boxed(ptr);
    })
}
