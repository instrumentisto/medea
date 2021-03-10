#![allow(clippy::not_unsafe_ptr_arg_deref, clippy::missing_safety_doc)]

// TODO: NPE check in Java (with wrapper) or Rust?
//       Exceptions object for each error (RoomError, MediaManagerError)
//       Catch panics in Jason worker and throw it as Exception

use std::{
    convert::TryFrom,
    marker::PhantomData,
    os::raw,
    ptr,
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;

use jni::objects::{GlobalRef, JObject, JValue};
use jni_sys::{
    jboolean, jclass, jfieldID, jfloat, jint, jlong, jmethodID, jstring,
    JNI_OK, JNI_VERSION_1_6,
};

use crate::{
    context::{JavaExecutor, RustExecutor},
    jni::util::{JNIEnv, JavaVM},
    AudioTrackConstraints, FacingMode, Jason, MediaKind, MediaSourceKind,
};
use jni::sys::jvalue;

mod audio_track_constraints;
mod connection_handle;
mod constraints_update_exception;
mod device_video_track_constraints;
mod display_video_track_constraints;
mod input_device_info;
mod jason;
mod jason_error;
mod local_media_track;
mod media_manager_handle;
mod media_stream_settings;
mod reconnect_handle;
mod remote_media_track;
mod room_close_reason;
mod room_handle;
pub mod util;

#[doc = " Default JNI_VERSION"]
const JNI_VERSION: jint = JNI_VERSION_1_6 as jint;

pub trait ForeignClass {
    fn box_object(self) -> jlong
    where
        Self: Sized,
    {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_boxed(ptr: jlong) -> Box<Self>
    where
        Self: Sized,
    {
        let this = Self::get_ptr(ptr);
        unsafe { Box::from_raw(this) }
    }

    fn get_ptr(ptr: jlong) -> *mut Self
    where
        Self: Sized,
    {
        unsafe { (ptr as *mut Self).as_mut().unwrap() }
    }
}

pub trait ForeignEnum {
    fn as_jint(&self) -> jint;

    fn from_jint(_: jint) -> Self;
}

pub struct JavaCallback<T> {
    consumer_object: GlobalRef,
    accept_method: jmethodID,
    _type: PhantomData<T>,
}

pub trait CallbackSignatuful {
    fn get_sig() -> &'static str;
}

impl CallbackSignatuful for () {
    fn get_sig() -> &'static str {
        "()V"
    }
}

impl CallbackSignatuful for i8 {
    fn get_sig() -> &'static str {
        "(S)V"
    }
}

impl CallbackSignatuful for u8 {
    fn get_sig() -> &'static str {
        "(S)V"
    }
}

impl CallbackSignatuful for i32 {
    fn get_sig() -> &'static str {
        "(I)V"
    }
}

impl CallbackSignatuful for i64 {
    fn get_sig() -> &'static str {
        "(J)V"
    }
}

impl<T: ForeignClass> CallbackSignatuful for T {
    fn get_sig() -> &'static str {
        "(Ljava/lang/Object;)V"
    }
}

impl<T: CallbackSignatuful> JavaCallback<T> {
    pub fn new(env: JNIEnv, obj: JObject) -> Self {
        let class = env.get_object_class(obj); // TODO: assert Consumer class
        assert!(!class.is_null(), "GetObjectClass return null class");
        // TODO: cache method?
        let accept_method =
            env.get_method_id(class, "accept", T::get_sig()).into_inner();

        let consumer_object = env.new_global_ref(obj);
        JavaCallback {
            consumer_object,
            accept_method,
            _type: PhantomData::default(),
        }
    }
}

macro_rules! primitive_java_callbacks {
    ( $( $primitive:ty ),* $(,)? ) => {
        $(
            impl JavaCallback<$primitive> {
                pub fn accept(self: Arc<Self>, arg: $primitive) {
                    exec_foreign(move |env| {
                        let arg = arg.into();
                        env.call_void_method(
                            self.consumer_object.as_obj(),
                            self.accept_method.into(),
                            &[arg],
                        );
                    });
                }
            }
        )*
    };
}

impl JavaCallback<()> {
    pub fn accept(self: Arc<Self>) {
        exec_foreign(move |env| {
            env.call_void_method(
                self.consumer_object.as_obj(),
                self.accept_method.into(),
                &[().into()],
            );
        });
    }
}

primitive_java_callbacks![bool, i8, u8, i32, i64];

impl<T: ForeignClass + Send + 'static> JavaCallback<T> {
    pub fn accept(self: Arc<Self>, arg: T) {
        exec_foreign(move |env| {
            let arg = arg.box_object().into();
            env.call_void_method(
                self.consumer_object.as_obj(),
                self.accept_method.into(),
                &[arg],
            );
        });
    }
}

/// Raw pointers are thread safe.
unsafe impl<T> Send for JavaCallback<T> {}

/// Raw pointers are thread safe.
unsafe impl<T> Sync for JavaCallback<T> {}

pub struct AsyncTaskCallback<T> {
    cb_object: GlobalRef,
    done_method: jmethodID,
    error_method: jmethodID,
    _type: PhantomData<T>,
}

impl<T> AsyncTaskCallback<T> {
    pub fn new(env: JNIEnv, obj: JObject) -> Self {
        let class = env.get_object_class(obj);
        assert!(!class.is_null(), "GetObjectClass return null class");

        let done_method = env
            .get_method_id(class, "onDone", "(Ljava/lang/Object;)V")
            .into_inner();
        let error_method = env
            .get_method_id(class, "onError", "(Ljava/lang/Throwable;)V")
            .into_inner();

        let cb_object = env.new_global_ref(obj);

        AsyncTaskCallback {
            cb_object,
            done_method,
            error_method,
            _type: PhantomData,
        }
    }

    pub fn reject(self) {
        unimplemented!()
    }
}

pub trait AsyncOutput {
    fn jvalue<'a>(&self) -> JValue<'a>;
}

impl AsyncOutput for () {
    fn jvalue<'a>(&self) -> JValue<'a> {
        JValue::Void
    }
}

impl<T: AsyncOutput + Send + 'static> AsyncTaskCallback<T> {
    pub fn resolve(self, arg: T) {
        exec_foreign(move |env| {
            let arg = arg.jvalue();
            env.call_void_method(
                self.cb_object.as_obj(),
                self.done_method.into(),
                &[arg],
            );
        });
    }
}

impl<T> Drop for AsyncTaskCallback<T> {
    fn drop(&mut self) {
        // TODO: DeleteGlobalRef(self.cb_object);
        // getting JNIEnv might be tricky here
    }
}

/// Raw pointers are thread safe.
unsafe impl<T> Send for AsyncTaskCallback<T> {}

/// Raw pointers are thread safe.
unsafe impl<T> Sync for AsyncTaskCallback<T> {}

impl ForeignEnum for FacingMode {
    fn as_jint(&self) -> jint {
        match *self {
            FacingMode::User => 0,
            FacingMode::Environment => 1,
            FacingMode::Left => 2,
            FacingMode::Right => 3,
        }
    }

    fn from_jint(x: jint) -> Self {
        match x {
            0 => FacingMode::User,
            1 => FacingMode::Environment,
            2 => FacingMode::Left,
            3 => FacingMode::Right,
            _ => panic!(
                concat!("{} not expected for ", stringify!(FacingMode)),
                x
            ),
        }
    }
}

impl ForeignEnum for MediaKind {
    fn as_jint(&self) -> jint {
        match *self {
            MediaKind::Audio => 0,
            MediaKind::Video => 1,
        }
    }

    fn from_jint(x: jint) -> Self {
        match x {
            0 => MediaKind::Audio,
            1 => MediaKind::Video,
            _ => panic!(
                concat!("{} not expected for ", stringify!(MediaKind)),
                x
            ),
        }
    }
}

impl ForeignEnum for MediaSourceKind {
    fn as_jint(&self) -> jint {
        match *self {
            MediaSourceKind::Device => 0,
            MediaSourceKind::Display => 1,
        }
    }

    fn from_jint(x: jint) -> Self {
        match x {
            0 => MediaSourceKind::Device,
            1 => MediaSourceKind::Display,
            _ => panic!(
                concat!("{} not expected for ", stringify!(MediaSourceKind)),
                x
            ),
        }
    }
}

struct Context {
    java: JavaExecutor,
    rust: RustExecutor,
}

impl Context {
    fn new(java_vm: JavaVM) -> Context {
        Self {
            java: JavaExecutor::new(java_vm),
            rust: RustExecutor::new(),
        }
    }
}

static CONTEXT: Lazy<Mutex<Option<Context>>> = Lazy::new(|| Mutex::new(None));

pub fn rust_exec_context() -> RustExecutor {
    CONTEXT.lock().unwrap().as_ref().unwrap().rust.clone()
}

pub fn exec_foreign<T>(task: T)
where
    T: FnOnce(JNIEnv) + Send + 'static,
{
    let executor = CONTEXT.lock().unwrap().as_ref().unwrap().java.clone();

    executor.execute(task);
}

#[no_mangle]
pub unsafe extern "system" fn JNI_OnLoad(
    java_vm: *mut jni_sys::JavaVM,
    _reserved: *mut raw::c_void,
) -> jint {
    // TODO: dont panic, log and return JNI_ERR.

    // It's ok to cache JavaVM, it is guaranteed to be valid until
    // `JNI_OnUnload`. In theory there may be multiple JavaVMs per process,
    // but Android only allows one.
    CONTEXT
        .lock()
        .unwrap()
        .replace(Context::new(JavaVM::from_raw(java_vm)));

    let mut env: *mut jni_sys::JNIEnv = ptr::null_mut();
    let res = (**java_vm).GetEnv.unwrap()(
        java_vm,
        (&mut env) as *mut *mut jni_sys::JNIEnv as *mut *mut raw::c_void,
        JNI_VERSION,
    );
    if res != (JNI_OK as jint) {
        panic!("JNI GetEnv in JNI_OnLoad failed, return code {}", res);
    }
    assert!(!env.is_null());

    JNI_VERSION
}

/// TODO: doesnt seem to fire on android for some reason, how do we stop exec
///       context (or maybe theres no need to)?
#[no_mangle]
pub unsafe extern "system" fn JNI_OnUnload(
    java_vm: *mut jni_sys::JavaVM,
    _reserved: *mut raw::c_void,
) {
    println!("JNI_OnUnloadJNI_OnUnloadJNI_OnUnloadJNI_OnUnload");
    log::error!("JNI_OnUnloadJNI_OnUnloadJNI_OnUnloadJNI_OnUnloadJNI_OnUnload");

    CONTEXT.lock().unwrap().take();

    assert!(!java_vm.is_null());
    let mut env: *mut jni_sys::JNIEnv = ptr::null_mut();
    let res = (**java_vm).GetEnv.unwrap()(
        java_vm,
        (&mut env) as *mut *mut jni_sys::JNIEnv as *mut *mut raw::c_void,
        JNI_VERSION,
    );
    if res != (JNI_OK as jint) {
        panic!("JNI GetEnv in JNI_OnLoad failed, return code {}", res);
    }
    assert!(!env.is_null());
}
