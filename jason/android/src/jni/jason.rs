use super::*;

use crate::RoomHandle;

impl ForeignClass for Jason {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_JASON }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_JASON_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_init(
    _: *mut JNIEnv,
    _: jclass,
) -> jlong {
    rust_exec_context().blocking_exec(move || Jason::new().box_object())
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeInitRoom(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
        this.init_room().box_object()
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeMediaManager(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    rust_exec_context().blocking_exec(move || {
        let this = unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
        this.media_manager().box_object()
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeCloseRoom(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    room_to_delete: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        let room_to_delete = unsafe {
            jlong_to_pointer::<RoomHandle>(room_to_delete)
                .as_mut()
                .unwrap()
        };
        let room_to_delete = unsafe { Box::from_raw(room_to_delete) };
        let this = unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
        this.close_room(*room_to_delete);
    })
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        Jason::get_boxed(ptr);
    })
}
