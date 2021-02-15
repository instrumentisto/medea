use super::*;

impl ForeignClass for RoomHandle {
    type PointedType = RoomHandle;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_ROOMHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_ROOMHANDLE_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        let this = Box::new(self);
        Box::into_raw(this) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(x).as_mut().unwrap() };
        ptr::NonNull::<Self::PointedType>::new(this).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeJoin(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    token: jstring,
) {
    let token = JavaString::new(env, token).to_str().to_owned();
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.join(token) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnNewConnection(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb: Box<dyn Consumer<ConnectionHandle>> =
        <Box<dyn Consumer<ConnectionHandle>>>::swig_from(cb, env);
    let this: &mut RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = RoomHandle::on_new_connection(this, cb) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnClose(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb = <Box<dyn Consumer<RoomCloseReason>>>::swig_from(cb, env);
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

    if let Err(msg) = this.on_close(cb) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnLocalTrack(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb = <Box<dyn Consumer<LocalMediaTrack>>>::swig_from(cb, env);
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.on_local_track(cb) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnFailedLocalMedia(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb = <Box<dyn Consumer<JasonError>>>::swig_from(cb, env);
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = RoomHandle::on_failed_local_media(this, cb) {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnConnectionLoss(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let cb = <Box<dyn Consumer<ReconnectHandle>>>::swig_from(cb, env);
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.on_connection_loss(cb) {
        jni_throw_exception(env, &msg);
    }
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

    if let Err(msg) =
        this.set_local_media_settings(settings, stop_first, rollback_on_fail)
    {
        jni_throw_exception(env, &msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeMuteAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: &RoomHandle =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

    if let Err(msg) = this.mute_audio() {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeUnmuteAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.unmute_audio() {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeMuteVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let source_kind = if source_kind == -1 {
        None
    } else {
        Some(<MediaSourceKind>::from_jint(source_kind))
    };
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.mute_video(source_kind) {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeUnmuteVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let source_kind = if source_kind == -1 {
        None
    } else {
        Some(<MediaSourceKind>::from_jint(source_kind))
    };
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.unmute_video(source_kind) {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.disable_audio() {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.enable_audio() {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let source_kind = if source_kind == -1 {
        None
    } else {
        Some(<MediaSourceKind>::from_jint(source_kind))
    };
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.disable_video(source_kind) {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let source_kind = if source_kind == -1 {
        None
    } else {
        Some(<MediaSourceKind>::from_jint(source_kind))
    };
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.enable_video(source_kind) {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableRemoteAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.disable_remote_audio() {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableRemoteVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.disable_remote_video() {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableRemoteAudio(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.enable_remote_audio() {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableRemoteVideo(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this =
        unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    if let Err(msg) = this.enable_remote_video() {
        jni_throw_exception(env, &msg)
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    RoomHandle::get_boxed(ptr);
}
