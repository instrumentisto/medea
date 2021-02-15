use super::*;

impl ForeignClass for DeviceVideoTrackConstraints {
    type PointedType = DeviceVideoTrackConstraints;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x: *mut DeviceVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeDeviceId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    device_id: jstring,
) {
    let device_id: JavaString = JavaString::new(env, device_id);
    let device_id: &str = device_id.to_str();
    let device_id: String = device_id.to_string();
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    this.device_id(device_id);
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactFacingMode(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    facing_mode: jint,
) {
    let facing_mode: FacingMode = <FacingMode>::from_jint(facing_mode);
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    this.exact_facing_mode(facing_mode);
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealFacingMode(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    facing_mode: jint,
) {
    let facing_mode: FacingMode = <FacingMode>::from_jint(facing_mode);
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    this.ideal_facing_mode(facing_mode);
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactHeight(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    height: jlong,
) {
    let height = u32::try_from(height)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    this.exact_height(height);
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealHeight(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    height: jlong,
) {
    let height = u32::try_from(height)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    this.ideal_height(height);
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeHeightInRange(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    min: jlong,
    max: jlong,
) {
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
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactWidth(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    width: jlong,
) {
    let width = u32::try_from(width)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    this.exact_width(width);
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealWidth(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    width: jlong,
) {
    let width = u32::try_from(width)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    this.ideal_width(width);
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeWidthInRange(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    min: jlong,
    max: jlong,
) {
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
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    DeviceVideoTrackConstraints::get_boxed(ptr);
}
