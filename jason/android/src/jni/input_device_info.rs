use super::*;

impl ForeignClass for InputDeviceInfo {
    type PointedType = InputDeviceInfo;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_INPUTDEVICEINFO }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_INPUTDEVICEINFO_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x: *mut InputDeviceInfo =
            unsafe { jlong_to_pointer::<InputDeviceInfo>(x).as_mut().unwrap() };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeDeviceId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let ret: String = InputDeviceInfo::device_id(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let ret: MediaKind = InputDeviceInfo::kind(this);
    let ret: jint = ret.as_jint();
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeLabel(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let ret: String = InputDeviceInfo::label(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeGroupId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let ret: String = InputDeviceInfo::group_id(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    InputDeviceInfo::get_boxed(ptr);
}
