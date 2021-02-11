use super::*;

impl ForeignClass for Jason {
    type PointedType = Jason;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_JASON }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_JASON_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<Jason> = Box::new(this);
        let this: *mut Jason = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut Jason =
            unsafe { jlong_to_pointer::<Jason>(x).as_mut().unwrap() };
        let x: Box<Jason> = unsafe { Box::from_raw(x) };
        let x: Jason = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut Jason =
            unsafe { jlong_to_pointer::<Jason>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_init(
    _: *mut JNIEnv,
    _: jclass,
) -> jlong {
    let this: Jason = Jason::new();
    let this: Box<Jason> = Box::new(this);
    let this: *mut Jason = Box::into_raw(this);
    this as jlong
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeInitRoom(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let this: &Jason =
        unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
    let ret: RoomHandle = Jason::init_room(this);
    let ret: jlong = <RoomHandle>::box_object(ret);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeMediaManager(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let this: &Jason =
        unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
    let ret: MediaManagerHandle = Jason::media_manager(this);
    let ret: jlong = <MediaManagerHandle>::box_object(ret);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeCloseRoom(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    room_to_delete: jlong,
) {
    let room_to_delete: *mut RoomHandle = unsafe {
        jlong_to_pointer::<RoomHandle>(room_to_delete)
            .as_mut()
            .unwrap()
    };
    let room_to_delete: Box<RoomHandle> =
        unsafe { Box::from_raw(room_to_delete) };
    let room_to_delete: RoomHandle = *room_to_delete;
    let this: &Jason =
        unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
    let ret: () = Jason::close_room(this, room_to_delete);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut Jason =
        unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
    let this: Box<Jason> = unsafe { Box::from_raw(this) };
    drop(this);
}
