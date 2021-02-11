use super::*;

impl ForeignClass for RoomCloseReason {
    type PointedType = RoomCloseReason;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_ROOMCLOSEREASON }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_ROOMCLOSEREASON_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<RoomCloseReason> = Box::new(this);
        let this: *mut RoomCloseReason = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut RoomCloseReason =
            unsafe { jlong_to_pointer::<RoomCloseReason>(x).as_mut().unwrap() };
        let x: Box<RoomCloseReason> = unsafe { Box::from_raw(x) };
        let x: RoomCloseReason = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut RoomCloseReason =
            unsafe { jlong_to_pointer::<RoomCloseReason>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeReason(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &RoomCloseReason =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    let ret: String = RoomCloseReason::reason(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeIsClosedByServer(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    let this: &RoomCloseReason =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    let ret: bool = RoomCloseReason::is_closed_by_server(this);
    ret as jboolean
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeRoomCloseReason(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    let this: &RoomCloseReason =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    let ret: bool = RoomCloseReason::is_err(this);
    ret as jboolean
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut RoomCloseReason =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    let this: Box<RoomCloseReason> = unsafe { Box::from_raw(this) };
    drop(this);
}
