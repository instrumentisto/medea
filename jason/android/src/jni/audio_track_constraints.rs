use jni_sys::{jclass, jlong};

use crate::{
    jni::{util::JNIEnv, ForeignClass},
    rust_exec_context, AudioTrackConstraints,
};
use jni::objects::JString;

impl ForeignClass for AudioTrackConstraints {}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeDeviceId(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    device_id: JString,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let device_id = env.clone_jstring_to_string(device_id);

    rust_exec_context().blocking_exec(move || {
        let mut this = unsafe { AudioTrackConstraints::get_ptr(this).as_mut().unwrap() };
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
