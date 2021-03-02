use super::*;

use crate::{MediaStreamSettings, RoomHandle};
use std::sync::Arc;

impl ForeignClass for RoomHandle {
    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_ROOMHANDLE }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_ROOMHANDLE_NATIVEPTR_FIELD }
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeJoin(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    token: jstring,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let token = env.clone_jstring_to_string(token);

    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.join(token).await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnNewConnection(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.on_new_connection(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnClose(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
        this.on_close(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnLocalTrack(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.on_local_track(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnFailedLocalMedia(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.on_failed_local_media(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnConnectionLoss(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    cb: jobject,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let cb = JavaCallback::new(env, cb);

    let result = rust_exec_context().blocking_exec(move || {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
        this.on_connection_loss(Arc::new(cb))
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeSetLocalMediaSettings(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    settings: jlong,
    stop_first: jboolean,
    rollback_on_fail: jboolean,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let settings = unsafe {
            jlong_to_pointer::<MediaStreamSettings>(settings)
                .as_mut()
                .unwrap()
        };
        let stop_first = stop_first != 0;
        let rollback_on_fail = rollback_on_fail != 0;

        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.set_local_media_settings(settings, stop_first, rollback_on_fail)
            .await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeMuteAudio(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.mute_audio().await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeUnmuteAudio(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.unmute_audio().await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeMuteVideo(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let source_kind = if source_kind == -1 {
            None
        } else {
            Some(MediaSourceKind::from_jint(source_kind))
        };
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.mute_video(source_kind).await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeUnmuteVideo(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let source_kind = if source_kind == -1 {
            None
        } else {
            Some(MediaSourceKind::from_jint(source_kind))
        };
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.unmute_video(source_kind).await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableAudio(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.disable_audio().await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableAudio(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.enable_audio().await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableVideo(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let source_kind = if source_kind == -1 {
            None
        } else {
            Some(MediaSourceKind::from_jint(source_kind))
        };
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.disable_video(source_kind).await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableVideo(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
    source_kind: jint,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let source_kind = if source_kind == -1 {
            None
        } else {
            Some(MediaSourceKind::from_jint(source_kind))
        };
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.enable_video(source_kind).await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableRemoteAudio(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.disable_remote_audio().await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeDisableRemoteVideo(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.disable_remote_video().await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableRemoteAudio(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.enable_remote_audio().await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeEnableRemoteVideo(
    env: *mut jni_sys::JNIEnv,
    _: jclass,
    this: jlong,
) {
    let env = unsafe { JNIEnv::from_raw(env) };
    let result = rust_exec_context().spawn_async(async move {
        let this =
            unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };

        this.enable_remote_video().await
    });

    if let Err(msg) = result {
        env.throw_new(&msg);
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeFree(
    _: *mut jni_sys::JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    rust_exec_context().blocking_exec(move || {
        RoomHandle::get_boxed(ptr);
    });
}
