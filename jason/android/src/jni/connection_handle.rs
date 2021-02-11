use super::*;

impl ForeignClass for ConnectionHandle {
    type PointedType = ConnectionHandle;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_CONNECTIONHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<ConnectionHandle> = Box::new(this);
        let this: *mut ConnectionHandle = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut ConnectionHandle = unsafe {
            jlong_to_pointer::<ConnectionHandle>(x).as_mut().unwrap()
        };
        let x: Box<ConnectionHandle> = unsafe { Box::from_raw(x) };
        let x: ConnectionHandle = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut ConnectionHandle = unsafe {
            jlong_to_pointer::<ConnectionHandle>(x).as_mut().unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnClose(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    f: jobject,
) {
    let f: Box<dyn Callback> = <Box<dyn Callback>>::swig_from(f, env);
    let this: &ConnectionHandle =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = ConnectionHandle::on_close(this, f);
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
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeGetRemoteMemberId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &ConnectionHandle =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    let ret: Result<String, String> =
        ConnectionHandle::get_remote_member_id(this);
    let ret: jstring = match ret {
        Ok(x) => {
            let ret: jstring = from_std_string_jstring(x, env);
            ret
        }
        Err(msg) => {
            jni_throw_exception(env, &msg);
            return <jstring>::jni_invalid_value();
        }
    };
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnRemoteTrackAdded(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    f: jobject,
) {
    let f: Box<dyn Consumer<RemoteMediaTrack>> =
        <Box<dyn Consumer<RemoteMediaTrack>>>::swig_from(f, env);
    let this: &ConnectionHandle =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> =
        ConnectionHandle::on_remote_track_added(this, f);
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
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeOnQualityScoreUpdate(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    f: jobject,
) {
    let f: Box<dyn Consumer<u8>> = <Box<dyn Consumer<u8>>>::swig_from(f, env);
    let this: &ConnectionHandle =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> =
        ConnectionHandle::on_quality_score_update(this, f);
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
pub extern "C" fn Java_com_jason_api_ConnectionHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut ConnectionHandle =
        unsafe { jlong_to_pointer::<ConnectionHandle>(this).as_mut().unwrap() };
    let this: Box<ConnectionHandle> = unsafe { Box::from_raw(this) };
    drop(this);
}
