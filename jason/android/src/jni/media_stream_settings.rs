use super::*;

use crate::{
    DeviceVideoTrackConstraints, DisplayVideoTrackConstraints,
    MediaStreamSettings,
};

impl ForeignClass for MediaStreamSettings {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_MEDIASTREAMSETTINGS }
    }

    fn native_ptr_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_MEDIASTREAMSETTINGS_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeAudio(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let constraints: *mut AudioTrackConstraints = unsafe {
            jlong_to_pointer::<AudioTrackConstraints>(constraints)
                .as_mut()
                .unwrap()
        };
        let constraints: Box<AudioTrackConstraints> =
            unsafe { Box::from_raw(constraints) };
        let this: &mut MediaStreamSettings = unsafe {
            jlong_to_pointer::<MediaStreamSettings>(this)
                .as_mut()
                .unwrap()
        };
        this.audio(*constraints)
    });
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeDeviceVideo(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let constraints: *mut DeviceVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(constraints)
                .as_mut()
                .unwrap()
        };
        let constraints: Box<DeviceVideoTrackConstraints> =
            unsafe { Box::from_raw(constraints) };
        let this: &mut MediaStreamSettings = unsafe {
            jlong_to_pointer::<MediaStreamSettings>(this)
                .as_mut()
                .unwrap()
        };
        this.device_video(*constraints);
    });
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeDisplayVideo(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let constraints: *mut DisplayVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DisplayVideoTrackConstraints>(constraints)
                .as_mut()
                .unwrap()
        };
        let constraints: Box<DisplayVideoTrackConstraints> =
            unsafe { Box::from_raw(constraints) };
        let this: &mut MediaStreamSettings = unsafe {
            jlong_to_pointer::<MediaStreamSettings>(this)
                .as_mut()
                .unwrap()
        };
        this.display_video(*constraints);
    });
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        MediaStreamSettings::get_boxed(ptr);
    });
}
