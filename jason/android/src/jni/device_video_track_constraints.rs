use super::*;

use crate::DeviceVideoTrackConstraints;

impl ForeignClass for DeviceVideoTrackConstraints {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS }
    }

    fn native_ptr_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeDeviceId(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    device_id: jstring,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let device_id = env.clone_jstring_to_string(device_id);

    rust_exec_context().blocking_exec(move || {
        let this: &mut DeviceVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.device_id(device_id);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactFacingMode(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    facing_mode: jint,
) {
    rust_exec_context().blocking_exec(move || {
        let facing_mode = FacingMode::from_jint(facing_mode);
        let this = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.exact_facing_mode(facing_mode);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealFacingMode(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    facing_mode: jint,
) {
    rust_exec_context().blocking_exec(move || {
        let facing_mode = FacingMode::from_jint(facing_mode);
        let this = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.ideal_facing_mode(facing_mode);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactHeight(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    height: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let height = u32::try_from(height)
            .expect("invalid jlong, in jlong => u32 conversation");
        let this = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.exact_height(height);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealHeight(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    height: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let height = u32::try_from(height)
            .expect("invalid jlong, in jlong => u32 conversation");
        let this = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.ideal_height(height);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeHeightInRange(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    min: jlong,
    max: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let min = u32::try_from(min)
            .expect("invalid jlong, in jlong => u32 conversation");
        let max = u32::try_from(max)
            .expect("invalid jlong, in jlong => u32 conversation");
        let this = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.height_in_range(min, max);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactWidth(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    width: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let width = u32::try_from(width)
            .expect("invalid jlong, in jlong => u32 conversation");
        let this = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.exact_width(width);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealWidth(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    width: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let width = u32::try_from(width)
            .expect("invalid jlong, in jlong => u32 conversation");
        let this = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.ideal_width(width);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeWidthInRange(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    min: jlong,
    max: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let min = u32::try_from(min)
            .expect("invalid jlong, in jlong => u32 conversation");
        let max = u32::try_from(max)
            .expect("invalid jlong, in jlong => u32 conversation");
        let this = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
                .as_mut()
                .unwrap()
        };
        this.width_in_range(min, max);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        DeviceVideoTrackConstraints::get_boxed(ptr);
    })
}
