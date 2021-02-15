use super::*;

impl ForeignClass for RoomCloseReason {
    type PointedType = RoomCloseReason;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_ROOMCLOSEREASON }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_ROOMCLOSEREASON_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x: *mut RoomCloseReason =
            unsafe { jlong_to_pointer::<RoomCloseReason>(x).as_mut().unwrap() };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeReason(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    from_std_string_jstring(this.reason(), env)
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
    ptr: jlong,
) {
    RoomCloseReason::get_boxed(ptr);
}
