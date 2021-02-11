use super::*;

impl ForeignClass for DeviceVideoTrackConstraints {
    type PointedType = DeviceVideoTrackConstraints;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<DeviceVideoTrackConstraints> = Box::new(this);
        let this: *mut DeviceVideoTrackConstraints = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut DeviceVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        let x: Box<DeviceVideoTrackConstraints> = unsafe { Box::from_raw(x) };
        let x: DeviceVideoTrackConstraints = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut DeviceVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
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
    let ret: () = DeviceVideoTrackConstraints::device_id(this, device_id);
    ret
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
    let ret: () =
        DeviceVideoTrackConstraints::exact_facing_mode(this, facing_mode);
    ret
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
    let ret: () =
        DeviceVideoTrackConstraints::ideal_facing_mode(this, facing_mode);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactHeight(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    height: jlong,
) {
    let height: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(height)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::exact_height(this, height);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealHeight(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    height: jlong,
) {
    let height: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(height)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::ideal_height(this, height);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeHeightInRange(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    min: jlong,
    max: jlong,
) {
    let min: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(min)
        .expect("invalid jlong, in jlong => u32 conversation");
    let max: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(max)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::height_in_range(this, min, max);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactWidth(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    width: jlong,
) {
    let width: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(width)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::exact_width(this, width);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealWidth(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    width: jlong,
) {
    let width: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(width)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::ideal_width(this, width);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeWidthInRange(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    min: jlong,
    max: jlong,
) {
    let min: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(min)
        .expect("invalid jlong, in jlong => u32 conversation");
    let max: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(max)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::width_in_range(this, min, max);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<DeviceVideoTrackConstraints> = unsafe { Box::from_raw(this) };
    drop(this);
}
