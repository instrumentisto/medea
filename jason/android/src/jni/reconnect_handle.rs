use super::*;

impl ForeignClass for ReconnectHandle {
    type PointedType = ReconnectHandle;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_RECONNECTHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_RECONNECTHANDLE_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x: *mut ReconnectHandle =
            unsafe { jlong_to_pointer::<ReconnectHandle>(x).as_mut().unwrap() };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ReconnectHandle_nativeReconnectWithDelay(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    delay_ms: jlong,
) {
    let delay_ms = u32::try_from(delay_ms)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this =
        unsafe { jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.reconnect_with_delay(delay_ms) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ReconnectHandle_nativeReconnectWithBackoff(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    starting_delay_ms: jlong,
    multiplier: jfloat,
    max_delay: jlong,
) {
    let starting_delay_ms = u32::try_from(starting_delay_ms)
        .expect("invalid jlong, in jlong => u32 conversation");
    let max_delay = u32::try_from(max_delay)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this =
        unsafe { jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap() };

    if let Err(msg) =
        this.reconnect_with_backoff(starting_delay_ms, multiplier, max_delay)
    {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ReconnectHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    ReconnectHandle::get_boxed(ptr);
}
