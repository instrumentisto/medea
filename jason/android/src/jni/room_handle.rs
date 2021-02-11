use super::*;

impl ForeignClass for RoomHandle {
    type PointedType = RoomHandle;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_ROOMHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_ROOMHANDLE_NATIVEPTR_FIELD }
    }

    fn box_object(this: Self) -> jlong {
        let this: Box<RoomHandle> = Box::new(this);
        let this: *mut RoomHandle = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut RoomHandle =
            unsafe { jlong_to_pointer::<RoomHandle>(x).as_mut().unwrap() };
        let x: Box<RoomHandle> = unsafe { Box::from_raw(x) };
        let x: RoomHandle = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut RoomHandle =
            unsafe { jlong_to_pointer::<RoomHandle>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeJoin(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    token: jstring,
) {
    let token: JavaString = JavaString::new(env, token);
    let token: &str = token.to_str();
    let token: String = token.to_string();
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::join(this, token);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnNewConnection(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<ConnectionHandle>> =
        <Box<dyn Consumer<ConnectionHandle>>>::swig_from(a0, env);
    let this: &mut RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::on_new_connection(this, a0);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnClose(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<RoomCloseReason>> =
        <Box<dyn Consumer<RoomCloseReason>>>::swig_from(a0, env);
    let this: &mut RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::on_close(this, a0);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnLocalTrack(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<LocalMediaTrack>> =
        <Box<dyn Consumer<LocalMediaTrack>>>::swig_from(a0, env);
    let this: &mut RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::on_local_track(this, a0);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnFailedLocalMedia(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<JasonError>> =
        <Box<dyn Consumer<JasonError>>>::swig_from(a0, env);
    let this: &mut RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::on_failed_local_media(this, a0);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnConnectionLoss(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<ReconnectHandle>> =
        <Box<dyn Consumer<ReconnectHandle>>>::swig_from(a0, env);
    let this: &mut RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::on_connection_loss(this, a0);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeSetLocalMediaSettings(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    settings: jlong,
    stop_first: jboolean,
    rollback_on_fail: jboolean,
) {
    let settings: &MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(settings)
            .as_mut()
            .unwrap()
    };
    let stop_first: bool = stop_first != 0;
    let rollback_on_fail: bool = rollback_on_fail != 0;
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::set_local_media_settings(
        this,
        settings,
        stop_first,
        rollback_on_fail,
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeMuteAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::mute_audio(this);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeUnmuteAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::unmute_audio(this);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeMuteVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let source_kind: Option<MediaSourceKind> = if source_kind != -1 {
        Some(<MediaSourceKind>::from_jint(source_kind))
    } else {
        None
    };
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::mute_video(this, source_kind);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeUnmuteVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let source_kind: Option<MediaSourceKind> = if source_kind != -1 {
        Some(<MediaSourceKind>::from_jint(source_kind))
    } else {
        None
    };
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::unmute_video(this, source_kind);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::disable_audio(this);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::enable_audio(this);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let source_kind: Option<MediaSourceKind> = if source_kind != -1 {
        Some(<MediaSourceKind>::from_jint(source_kind))
    } else {
        None
    };
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::disable_video(this, source_kind);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let source_kind: Option<MediaSourceKind> = if source_kind != -1 {
        Some(<MediaSourceKind>::from_jint(source_kind))
    } else {
        None
    };
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::enable_video(this, source_kind);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableRemoteAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::disable_remote_audio(this);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableRemoteVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::disable_remote_video(this);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableRemoteAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::enable_remote_audio(this);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableRemoteVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = RoomHandle::enable_remote_video(this);
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let this: Box<RoomHandle> = unsafe { Box::from_raw(this) };
    drop(this);
}
