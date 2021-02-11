use super::*;

impl ForeignClass for ReconnectHandle {
    type PointedType = ReconnectHandle;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_RECONNECTHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_RECONNECTHANDLE_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<ReconnectHandle> = Box::new(this);
        let this: *mut ReconnectHandle = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut ReconnectHandle =
            unsafe { jlong_to_pointer::<ReconnectHandle>(x).as_mut().unwrap() };
        let x: Box<ReconnectHandle> = unsafe { Box::from_raw(x) };
        let x: ReconnectHandle = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut ReconnectHandle =
            unsafe { jlong_to_pointer::<ReconnectHandle>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ReconnectHandle_nativeReconnectWithDelay(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    delay_ms: jlong,
) {
    let delay_ms: u32 =
        <u32 as ::std::convert::TryFrom<jlong>>::try_from(delay_ms)
            .expect("invalid jlong, in jlong => u32 conversation");
    let this: &ReconnectHandle =
        unsafe { jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> =
        ReconnectHandle::reconnect_with_delay(this, delay_ms);
    let ret: () = match ret {
        Ok(x) => x,
        Err(msg) => {
            jni_throw_exception(env, &msg);
            return <()>::jni_invalid_value();
        }
    };
    ret
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
    let starting_delay_ms: u32 =
        <u32 as ::std::convert::TryFrom<jlong>>::try_from(starting_delay_ms)
            .expect("invalid jlong, in jlong => u32 conversation");
    let multiplier: f32 = multiplier;
    let max_delay: u32 =
        <u32 as ::std::convert::TryFrom<jlong>>::try_from(max_delay)
            .expect("invalid jlong, in jlong => u32 conversation");
    let this: &ReconnectHandle =
        unsafe { jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = ReconnectHandle::reconnect_with_backoff(
        this,
        starting_delay_ms,
        multiplier,
        max_delay,
    );
    let ret: () = match ret {
        Ok(x) => x,
        Err(msg) => {
            jni_throw_exception(env, &msg);
            return <()>::jni_invalid_value();
        }
    };
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ReconnectHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut ReconnectHandle =
        unsafe { jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap() };
    let this: Box<ReconnectHandle> = unsafe { Box::from_raw(this) };
    drop(this);
}
