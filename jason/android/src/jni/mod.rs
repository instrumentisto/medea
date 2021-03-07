#![allow(clippy::not_unsafe_ptr_arg_deref, clippy::missing_safety_doc)]

use std::{
    convert::TryFrom,
    marker::PhantomData,
    os::raw,
    ptr,
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;

use jni_sys::{
    jboolean, jclass, jfieldID, jfloat, jint, jlong, jmethodID, jobject,
    jstring, jvalue, JNI_OK, JNI_VERSION_1_6,
};

use crate::{
    context::{JavaExecutor, RustExecutor},
    jni::util::{JNIEnv, JavaVM},
    AudioTrackConstraints, FacingMode, Jason, MediaKind, MediaSourceKind,
};

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

macro_rules! as_c_str {
    ($lit:expr) => {
        concat!($lit, "\0").as_ptr() as *const ::std::os::raw::c_char
    };
}

#[cfg(target_pointer_width = "32")]
pub unsafe fn jlong_to_pointer<T>(val: jlong) -> *mut T {
    (val as u32) as *mut T
}

#[cfg(target_pointer_width = "64")]
pub unsafe fn jlong_to_pointer<T>(val: jlong) -> *mut T {
    val as *mut T
}

pub trait IntoJValue {
    fn into_jvalue(self) -> jvalue;
}

impl IntoJValue for () {
    fn into_jvalue(self) -> jvalue {
        jvalue { l: ptr::null_mut() }
    }
}

impl IntoJValue for jobject {
    fn into_jvalue(self) -> jvalue {
        jvalue {
            l: unsafe { std::mem::transmute(self) },
        }
    }
}

// impl IntoJValue for i8 {
//     fn into_jvalue(self) -> jvalue {
//         jvalue { b: self }
//     }
// }
//
impl IntoJValue for u8 {
    fn into_jvalue(self) -> jvalue {
        (self as i32).into_jvalue()
    }
}
// impl IntoJValue for char {
//     fn into_jvalue(self) -> jvalue {
//         jvalue { c: self as u16 }
//     }
// }
//
// impl IntoJValue for i16 {
//     fn into_jvalue(self) -> jvalue {
//         jvalue { s: self }
//     }
// }
//
impl IntoJValue for i32 {
    fn into_jvalue(self) -> jvalue {
        jvalue { i: self }
    }
}

impl IntoJValue for i64 {
    fn into_jvalue(self) -> jvalue {
        jvalue { j: self }
    }
}
//
// impl IntoJValue for bool {
//     fn into_jvalue(self) -> jvalue {
//         jvalue { b: self as i8 }
//     }
// }
//
// impl IntoJValue for f32 {
//     fn into_jvalue(self) -> jvalue {
//         jvalue { f: self }
//     }
// }
//
// impl IntoJValue for f64 {
//     fn into_jvalue(self) -> jvalue {
//         jvalue { d: self }
//     }
// }

// TODO: Remove. Dont alloc java object from rust, pass pointers that java will wrap in java object.
//       Same for JForeignObjectsArray.
trait IntoJObject {
    fn into_jobject(self, env: JNIEnv) -> jobject;
}

impl<T: ForeignClass> IntoJObject for T {
    fn into_jobject(self, env: JNIEnv) -> jobject {
        let jclass = <T>::jni_class();
        assert!(!jclass.is_null());
        let field_id = <T>::native_ptr_field();
        assert!(!field_id.is_null());
        let jobj = env.alloc_object(jclass);
        assert!(!jobj.is_null(), "object_to_jobject: AllocObject failed");
        let native_ptr: jlong = <T>::box_object(self);
        env.set_long_field(jobj, field_id, native_ptr);
        if env.exception_check() {
            env.exception_describe();
            panic!(
                "object_to_jobject: Can not set nativePtr field: catch \
                 exception"
            )
        }

        jobj
    }
}

pub trait ForeignClass {
    fn jni_class() -> jclass;

    fn native_ptr_field() -> jfieldID;

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
        let this = Self::get_ptr(ptr).as_ptr();
        unsafe { Box::from_raw(this) }
    }

    fn get_ptr(ptr: jlong) -> ptr::NonNull<Self>
    where
        Self: Sized,
    {
        let this = unsafe { jlong_to_pointer::<Self>(ptr).as_mut().unwrap() };
        ptr::NonNull::new(this).unwrap()
    }
}

pub trait ForeignEnum {
    fn as_jint(&self) -> jint;

    fn from_jint(_: jint) -> Self;
}

pub struct JavaCallback<T> {
    consumer_object: jobject,
    accept_method: jmethodID,
    _type: PhantomData<T>,
}

impl<T> JavaCallback<T> {
    pub fn new(env: JNIEnv, obj: jobject) -> Self {
        let class = env.get_object_class(obj); // TODO: assert Consumer class
        assert!(!class.is_null(), "GetObjectClass return null class");
        // TODO: cache method?
        let accept_method =
            env.get_method_id(class, as_c_str!("accept"), as_c_str!("(J)V"));
        assert!(!accept_method.is_null(), "Can not find accept id");

        let consumer_object = env.new_global_ref(obj);
        assert!(!consumer_object.is_null());
        JavaCallback {
            consumer_object,
            accept_method,
            _type: PhantomData::default(),
        }
    }
}

impl JavaCallback<()> {
    pub fn accept(self: Arc<Self>, void: ()) {
        exec_foreign(move |env| {
            env.call_void_method(
                self.consumer_object,
                self.accept_method,
                &[void.into_jvalue()],
            );
        });
    }
}

impl JavaCallback<u8> {
    pub fn accept(self: Arc<Self>, arg: u8) {
        exec_foreign(move |env| {
            let arg = arg.into_jvalue();
            env.call_void_method(
                self.consumer_object,
                self.accept_method,
                &[arg],
            );
        });
    }
}

impl<T: ForeignClass + Send + 'static> JavaCallback<T> {
    pub fn accept(self: Arc<Self>, arg: T) {
        exec_foreign(move |env| {
            let arg = arg.box_object().into_jvalue();
            env.call_void_method(
                self.consumer_object,
                self.accept_method,
                &[arg],
            );
        });
    }
}

// impl<T: IntoJObject + Send + 'static> JavaCallback<T> {
//     pub fn accept(self: Arc<Self>, arg: T) {
//         exec_foreign(move |env| {
//             let arg = arg.into_jobject(env).into_jvalue();
//             env.call_void_method(
//                 self.consumer_object,
//                 self.accept_method,
//                 &[arg],
//             );
//         });
//     }
// }
//
// impl<T: IntoJValue + Send + 'static> JavaCallback<T> {
//     pub fn accept(self: Arc<Self>, arg: T) {
//         exec_foreign(move |env| {
//             let arg = arg.into_jvalue();
//             env.call_void_method(
//                 self.consumer_object,
//                 self.accept_method,
//                 &[arg],
//             );
//         });
//     }
// }

impl<T> Drop for JavaCallback<T> {
    fn drop(&mut self) {
        // TODO: DeleteGlobalRef(self.consumer_object);
        // getting JNIEnv might be tricky here
    }
}

/// Raw pointers are thread safe.
unsafe impl<T> Send for JavaCallback<T> {}

/// Raw pointers are thread safe.
unsafe impl<T> Sync for JavaCallback<T> {}

pub struct AsyncTaskCallback<T> {
    cb_object: jobject,
    done_method: jmethodID,
    error_method: jmethodID,
    _type: PhantomData<T>,
}

impl<T> AsyncTaskCallback<T> {
    pub fn new(env: JNIEnv, obj: jobject) -> Self {
        let class = env.get_object_class(obj);
        assert!(!class.is_null(), "GetObjectClass return null class");

        let done_method = env.get_method_id(
            class,
            as_c_str!("onDone"),
            as_c_str!("(Ljava/lang/Object;)V"),
        );
        let error_method = env.get_method_id(
            class,
            as_c_str!("onError"),
            as_c_str!("(Ljava/lang/Throwable;)V"),
        );

        assert!(!done_method.is_null(), "Can not find onDone id");
        assert!(!error_method.is_null(), "Can not find onError id");

        let cb_object = env.new_global_ref(obj);
        assert!(!cb_object.is_null());

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

impl AsyncTaskCallback<()> {
    pub fn resolve(self, void: ()) {
        exec_foreign(move |env| {
            env.call_void_method(
                self.cb_object,
                self.done_method,
                &[void.into_jvalue()],
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

impl IntoJObject for FacingMode {
    fn into_jobject(self, env: JNIEnv) -> jobject {
        let class = unsafe { FOREIGN_ENUM_FACINGMODE };
        assert!(!class.is_null());
        let field_id: jfieldID = match self {
            FacingMode::User => {
                let field = unsafe { FOREIGN_ENUM_FACINGMODE_USER };
                assert!(!field.is_null());
                field
            }
            FacingMode::Environment => {
                let field = unsafe { FOREIGN_ENUM_FACINGMODE_IRONMENT };
                assert!(!field.is_null());
                field
            }
            FacingMode::Left => {
                let field = unsafe { FOREIGN_ENUM_FACINGMODE_LEFT };
                assert!(!field.is_null());
                field
            }
            FacingMode::Right => {
                let field = unsafe { FOREIGN_ENUM_FACINGMODE_RIGHT };
                assert!(!field.is_null());
                field
            }
        };
        assert!(!field_id.is_null());
        let obj = env.get_static_object_field(class, field_id);
        assert!(
            !obj.is_null(),
            concat!("Can get value of item in ", "com/jason/api/FacingMode")
        );
        obj
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

impl IntoJObject for MediaKind {
    fn into_jobject(self, env: JNIEnv) -> jobject {
        let class = unsafe { FOREIGN_ENUM_MEDIAKIND };
        assert!(!class.is_null());
        let field: jfieldID = match self {
            MediaKind::Audio => {
                let field = unsafe { FOREIGN_ENUM_MEDIAKIND_AUDIO };
                assert!(!field.is_null());
                field
            }
            MediaKind::Video => {
                let field = unsafe { FOREIGN_ENUM_MEDIAKIND_VIDEO };
                assert!(!field.is_null());
                field
            }
        };
        assert!(!field.is_null());
        let obj = env.get_static_object_field(class, field);
        assert!(
            !obj.is_null(),
            concat!("Can get value of item in ", "com/jason/api/MediaKind")
        );
        obj
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

impl IntoJObject for MediaSourceKind {
    fn into_jobject(self, env: JNIEnv) -> jobject {
        let class = unsafe { FOREIGN_ENUM_MEDIASOURCEKIND };
        assert!(!class.is_null());
        let field_id = match self {
            MediaSourceKind::Device => {
                let field = unsafe { FOREIGN_ENUM_MEDIASOURCEKIND_DEVICE };
                assert!(!field.is_null());
                field
            }
            MediaSourceKind::Display => {
                let field = unsafe { FOREIGN_ENUM_MEDIASOURCEKIND_DISPLAY };
                assert!(!field.is_null());
                field
            }
        };
        assert!(!field_id.is_null());
        let obj = env.get_static_object_field(class, field_id);
        assert!(
            !obj.is_null(),
            concat!(
                "Can get value of item in ",
                "com/jason/api/MediaSourceKind"
            )
        );
        obj
    }
}

macro_rules! cache_foreign_class {
    ($env:ident; $class:expr => $path:expr) => {
            let class_local_ref = (**$env).FindClass.unwrap()(
                $env,
                as_c_str!($path),
            );
            assert!(
                !class_local_ref.is_null(),
                concat!("FindClass failed for ", $path)
            );
            let class = (**$env).NewGlobalRef.unwrap()($env, class_local_ref);
            assert!(
                !class.is_null(),
                concat!( "FindClass failed for ", $path)
            );
            (**$env).DeleteLocalRef.unwrap()($env, class_local_ref);
            $class = class;
    };
    ($env:ident; $class:expr => $path:expr, {$($field:expr => $field_name:expr => $field_sig:expr),*}) => {
            cache_foreign_class!($env; $class => $path);
            $(
                let field_id: jfieldID = (**$env).GetFieldID.unwrap()(
                    $env,
                    $class,
                    as_c_str!($field_name),
                    as_c_str!($field_sig),
                );
                assert!(
                    !field_id.is_null(),
                    concat!("GetFieldID for class ", $path, " field ",
                        $field_name, " with sig ", $field_sig, " failed")
                );
                $field = field_id;
            )*
    };
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

static mut JAVA_LANG_LONG: jclass = ptr::null_mut();
static mut JAVA_LANG_LONG_LONG_VALUE: jmethodID = ptr::null_mut();
static mut JAVA_LANG_EXCEPTION: jclass = ptr::null_mut();
static mut JAVA_LANG_FLOAT: jclass = ptr::null_mut();
static mut JAVA_LANG_FLOAT_FLOAT_VALUE: jmethodID = ptr::null_mut();
static mut JAVA_LANG_STRING: jclass = ptr::null_mut();
static mut JAVA_LANG_SHORT: jclass = ptr::null_mut();
static mut JAVA_LANG_SHORT_SHORT_VALUE: jmethodID = ptr::null_mut();
static mut JAVA_LANG_DOUBLE: jclass = ptr::null_mut();
static mut JAVA_LANG_DOUBLE_DOUBLE_VALUE_METHOD: jmethodID = ptr::null_mut();
static mut JAVA_LANG_BYTE: jclass = ptr::null_mut();
static mut JAVA_LANG_BYTE_BYTE_VALUE: jmethodID = ptr::null_mut();
static mut JAVA_LANG_INTEGER: jclass = ptr::null_mut();
static mut JAVA_LANG_INTEGER_INT_VALUE: jmethodID = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_INT: jclass = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_INT_OF: jmethodID = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_INT_EMPTY: jmethodID = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE: jclass = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE_OF: jmethodID = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE_EMPTY: jmethodID = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG: jclass = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG_OF: jmethodID = ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG_EMPTY: jmethodID = ptr::null_mut();
static mut FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_REMOTEMEDIATRACK: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_REMOTEMEDIATRACK_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_JASONERROR: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_JASONERROR_NATIVEPTR_FIELD: jfieldID = ptr::null_mut();
static mut FOREIGN_CLASS_MEDIAMANAGERHANDLE: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_MEDIAMANAGERHANDLE_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_ROOMCLOSEREASON: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_ROOMCLOSEREASON_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_RECONNECTHANDLE: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_RECONNECTHANDLE_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_MEDIASTREAMSETTINGS: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_MEDIASTREAMSETTINGS_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_LOCALMEDIATRACK: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_LOCALMEDIATRACK_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_JASON: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_JASON_NATIVEPTR_FIELD: jfieldID = ptr::null_mut();
static mut FOREIGN_CLASS_CONNECTIONHANDLE: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD:
    jfieldID = ptr::null_mut();
static mut FOREIGN_CLASS_ROOMHANDLE: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_ROOMHANDLE_NATIVEPTR_FIELD: jfieldID = ptr::null_mut();
static mut FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_CLASS_INPUTDEVICEINFO: jclass = ptr::null_mut();
static mut FOREIGN_CLASS_INPUTDEVICEINFO_NATIVEPTR_FIELD: jfieldID =
    ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE: jclass = ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_USER: jfieldID = ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_IRONMENT: jfieldID = ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_LEFT: jfieldID = ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_RIGHT: jfieldID = ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND: jclass = ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND_AUDIO: jfieldID = ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND_VIDEO: jfieldID = ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND: jclass = ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND_DEVICE: jfieldID = ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND_DISPLAY: jfieldID = ptr::null_mut();

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

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/lang/Exception"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/Exception"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/Exception");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_EXCEPTION = class;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/lang/Long"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/Long"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/Long");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_LONG = class;

    let method_id: jmethodID = (**env).GetMethodID.unwrap()(
        env,
        class,
        as_c_str!("longValue"),
        as_c_str!("()J"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Long method longValue sig ()J failed"
    );
    JAVA_LANG_LONG_LONG_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/lang/Byte"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/Byte"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/Byte");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_BYTE = class;
    let method_id: jmethodID = (**env).GetMethodID.unwrap()(
        env,
        class,
        as_c_str!("byteValue"),
        as_c_str!("()B"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Byte method byteValue sig ()B failed"
    );
    JAVA_LANG_BYTE_BYTE_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/lang/Integer"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/Integer"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/Integer");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_INTEGER = class;
    let method_id: jmethodID = (**env).GetMethodID.unwrap()(
        env,
        class,
        as_c_str!("intValue"),
        as_c_str!("()I"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Integer method intValue sig ()I \
         failed"
    );
    JAVA_LANG_INTEGER_INT_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/lang/Short"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/Short"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/Short");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_SHORT = class;
    let method_id: jmethodID = (**env).GetMethodID.unwrap()(
        env,
        class,
        as_c_str!("shortValue"),
        as_c_str!("()S"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Short method shortValue sig ()S \
         failed"
    );
    JAVA_LANG_SHORT_SHORT_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/lang/Double"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/Double"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/Double");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_DOUBLE = class;
    let method_id: jmethodID = (**env).GetMethodID.unwrap()(
        env,
        class,
        as_c_str!("doubleValue"),
        as_c_str!("()D"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Double method doubleValue sig ()D \
         failed"
    );
    JAVA_LANG_DOUBLE_DOUBLE_VALUE_METHOD = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/lang/String"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/String"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/String");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_STRING = class;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/lang/Float"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/Float"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/Float");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_FLOAT = class;
    let method_id: jmethodID = (**env).GetMethodID.unwrap()(
        env,
        class,
        as_c_str!("floatValue"),
        as_c_str!("()F"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Float method floatValue sig ()F \
         failed"
    );
    JAVA_LANG_FLOAT_FLOAT_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/util/OptionalDouble"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/util/OptionalDouble"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(
        !class.is_null(),
        "FindClass failed for java/util/OptionalDouble"
    );
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_UTIL_OPTIONAL_DOUBLE = class;
    let method_id: jmethodID = (**env).GetStaticMethodID.unwrap()(
        env,
        class,
        as_c_str!("of"),
        as_c_str!("(D)Ljava/util/OptionalDouble;"),
    );
    assert!(
        !method_id.is_null(),
        "GetStaticMethodID for class java/util/OptionalDouble method of sig \
         (D)Ljava/util/OptionalDouble; failed"
    );
    JAVA_UTIL_OPTIONAL_DOUBLE_OF = method_id;
    let method_id: jmethodID = (**env).GetStaticMethodID.unwrap()(
        env,
        class,
        as_c_str!("empty"),
        as_c_str!("()Ljava/util/OptionalDouble;"),
    );
    assert!(
        !method_id.is_null(),
        "GetStaticMethodID for class java/util/OptionalDouble method empty \
         sig ()Ljava/util/OptionalDouble; failed"
    );
    JAVA_UTIL_OPTIONAL_DOUBLE_EMPTY = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/util/OptionalInt"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/util/OptionalInt"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(
        !class.is_null(),
        "FindClass failed for java/util/OptionalInt"
    );
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_UTIL_OPTIONAL_INT = class;
    let method_id: jmethodID = (**env).GetStaticMethodID.unwrap()(
        env,
        class,
        as_c_str!("of"),
        as_c_str!("(I)Ljava/util/OptionalInt;"),
    );
    assert!(
        !method_id.is_null(),
        "GetStaticMethodID for class java/util/OptionalInt method of sig \
         (I)Ljava/util/OptionalInt; failed"
    );
    JAVA_UTIL_OPTIONAL_INT_OF = method_id;
    let method_id: jmethodID = (**env).GetStaticMethodID.unwrap()(
        env,
        class,
        as_c_str!("empty"),
        as_c_str!("()Ljava/util/OptionalInt;"),
    );
    assert!(
        !method_id.is_null(),
        "GetStaticMethodID for class java/util/OptionalInt method empty sig \
         ()Ljava/util/OptionalInt; failed"
    );
    JAVA_UTIL_OPTIONAL_INT_EMPTY = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("java/util/OptionalLong"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/util/OptionalLong"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(
        !class.is_null(),
        "FindClass failed for java/util/OptionalLong"
    );
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_UTIL_OPTIONAL_LONG = class;
    let method_id: jmethodID = (**env).GetStaticMethodID.unwrap()(
        env,
        class,
        as_c_str!("of"),
        as_c_str!("(J)Ljava/util/OptionalLong;"),
    );
    assert!(
        !method_id.is_null(),
        "GetStaticMethodID for class java/util/OptionalLong method of sig \
         (J)Ljava/util/OptionalLong; failed"
    );
    JAVA_UTIL_OPTIONAL_LONG_OF = method_id;
    let method_id: jmethodID = (**env).GetStaticMethodID.unwrap()(
        env,
        class,
        as_c_str!("empty"),
        as_c_str!("()Ljava/util/OptionalLong;"),
    );
    assert!(
        !method_id.is_null(),
        "GetStaticMethodID for class java/util/OptionalLong method empty sig \
         ()Ljava/util/OptionalLong; failed"
    );
    JAVA_UTIL_OPTIONAL_LONG_EMPTY = method_id;

    cache_foreign_class!(env;
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION => "com/jason/api/ConstraintsUpdateException",
        {
            FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_REMOTEMEDIATRACK => "com/jason/api/RemoteMediaTrack",
        {
            FOREIGN_CLASS_REMOTEMEDIATRACK_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_JASONERROR => "com/jason/api/JasonError",
        {
            FOREIGN_CLASS_JASONERROR_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_MEDIAMANAGERHANDLE => "com/jason/api/MediaManagerHandle",
        {
            FOREIGN_CLASS_MEDIAMANAGERHANDLE_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_ROOMCLOSEREASON => "com/jason/api/RoomCloseReason",
        {
            FOREIGN_CLASS_ROOMCLOSEREASON_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_RECONNECTHANDLE => "com/jason/api/ReconnectHandle",
        {
            FOREIGN_CLASS_RECONNECTHANDLE_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_MEDIASTREAMSETTINGS => "com/jason/api/MediaStreamSettings",
        {
            FOREIGN_CLASS_MEDIASTREAMSETTINGS_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_LOCALMEDIATRACK => "com/jason/api/LocalMediaTrack",
        {
            FOREIGN_CLASS_LOCALMEDIATRACK_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_JASON => "com/jason/api/Jason",
        {
            FOREIGN_CLASS_JASON_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_CONNECTIONHANDLE => "com/jason/api/ConnectionHandle",
        {
            FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS => "com/jason/api/DisplayVideoTrackConstraints",
        {
            FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_ROOMHANDLE => "com/jason/api/RoomHandle",
        {
            FOREIGN_CLASS_ROOMHANDLE_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS => "com/jason/api/AudioTrackConstraints",
        {
            FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );
    cache_foreign_class!(env;
        FOREIGN_CLASS_INPUTDEVICEINFO => "com/jason/api/InputDeviceInfo",
        {
            FOREIGN_CLASS_INPUTDEVICEINFO_NATIVEPTR_FIELD => "nativePtr" => "J"
        }
    );

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("com/jason/api/FacingMode"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for com/jason/api/FacingMode"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(
        !class.is_null(),
        "FindClass failed for com/jason/api/FacingMode"
    );
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    FOREIGN_ENUM_FACINGMODE = class;
    let field_id = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        as_c_str!("User"),
        as_c_str!("Lcom/jason/api/FacingMode;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/FacingMode method User sig \
         Lcom/jason/api/FacingMode; failed"
    );
    FOREIGN_ENUM_FACINGMODE_USER = field_id;
    let field_id = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        as_c_str!("Environment"),
        as_c_str!("Lcom/jason/api/FacingMode;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/FacingMode method \
         Environment sig Lcom/jason/api/FacingMode; failed"
    );
    FOREIGN_ENUM_FACINGMODE_IRONMENT = field_id;
    let field_id: jfieldID = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        as_c_str!("Left"),
        as_c_str!("Lcom/jason/api/FacingMode;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/FacingMode method Left sig \
         Lcom/jason/api/FacingMode; failed"
    );
    FOREIGN_ENUM_FACINGMODE_LEFT = field_id;
    let field_id: jfieldID = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        as_c_str!("Right"),
        as_c_str!("Lcom/jason/api/FacingMode;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/FacingMode method Right sig \
         Lcom/jason/api/FacingMode; failed"
    );
    FOREIGN_ENUM_FACINGMODE_RIGHT = field_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, as_c_str!("com/jason/api/MediaKind"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for com/jason/api/MediaKind"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(
        !class.is_null(),
        "FindClass failed for com/jason/api/MediaKind"
    );
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    FOREIGN_ENUM_MEDIAKIND = class;
    let field_id: jfieldID = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        as_c_str!("Audio"),
        as_c_str!("Lcom/jason/api/MediaKind;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/MediaKind method Audio sig \
         Lcom/jason/api/MediaKind; failed"
    );
    FOREIGN_ENUM_MEDIAKIND_AUDIO = field_id;
    let field_id: jfieldID = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        as_c_str!("Video"),
        as_c_str!("Lcom/jason/api/MediaKind;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/MediaKind method Video sig \
         Lcom/jason/api/MediaKind; failed"
    );
    FOREIGN_ENUM_MEDIAKIND_VIDEO = field_id;

    let class_local_ref = (**env).FindClass.unwrap()(
        env,
        as_c_str!("com/jason/api/MediaSourceKind"),
    );
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for com/jason/api/MediaSourceKind"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(
        !class.is_null(),
        "FindClass failed for com/jason/api/MediaSourceKind"
    );
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    FOREIGN_ENUM_MEDIASOURCEKIND = class;
    let field_id: jfieldID = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        as_c_str!("Device"),
        as_c_str!("Lcom/jason/api/MediaSourceKind;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/MediaSourceKind method \
         Device sig Lcom/jason/api/MediaSourceKind; failed"
    );
    FOREIGN_ENUM_MEDIASOURCEKIND_DEVICE = field_id;
    let field_id: jfieldID = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        as_c_str!("Display"),
        as_c_str!("Lcom/jason/api/MediaSourceKind;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/MediaSourceKind method \
         Display sig Lcom/jason/api/MediaSourceKind; failed"
    );
    FOREIGN_ENUM_MEDIASOURCEKIND_DISPLAY = field_id;

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

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_SHORT);
    JAVA_LANG_SHORT = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_DOUBLE);
    JAVA_LANG_DOUBLE = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_BYTE);
    JAVA_LANG_BYTE = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_INTEGER);
    JAVA_LANG_INTEGER = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_LONG);
    JAVA_LANG_LONG = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_EXCEPTION);
    JAVA_LANG_EXCEPTION = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_FLOAT);
    JAVA_LANG_FLOAT = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_STRING);
    JAVA_LANG_STRING = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_UTIL_OPTIONAL_INT);
    JAVA_UTIL_OPTIONAL_INT = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_UTIL_OPTIONAL_DOUBLE);
    JAVA_UTIL_OPTIONAL_DOUBLE = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, JAVA_UTIL_OPTIONAL_LONG);
    JAVA_UTIL_OPTIONAL_LONG = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(
        env,
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION,
    );
    FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_REMOTEMEDIATRACK);
    FOREIGN_CLASS_REMOTEMEDIATRACK = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_JASONERROR);
    FOREIGN_CLASS_JASONERROR = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_MEDIAMANAGERHANDLE);
    FOREIGN_CLASS_MEDIAMANAGERHANDLE = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_ROOMCLOSEREASON);
    FOREIGN_CLASS_ROOMCLOSEREASON = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_RECONNECTHANDLE);
    FOREIGN_CLASS_RECONNECTHANDLE = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_MEDIASTREAMSETTINGS);
    FOREIGN_CLASS_MEDIASTREAMSETTINGS = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_ENUM_FACINGMODE);
    FOREIGN_ENUM_FACINGMODE = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_LOCALMEDIATRACK);
    FOREIGN_CLASS_LOCALMEDIATRACK = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(
        env,
        FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS,
    );
    FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_JASON);
    FOREIGN_CLASS_JASON = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_CONNECTIONHANDLE);
    FOREIGN_CLASS_CONNECTIONHANDLE = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_ENUM_MEDIAKIND);
    FOREIGN_ENUM_MEDIAKIND = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(
        env,
        FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS,
    );
    FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_ENUM_MEDIASOURCEKIND);
    FOREIGN_ENUM_MEDIASOURCEKIND = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_ROOMHANDLE);
    FOREIGN_CLASS_ROOMHANDLE = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS);
    FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS = ptr::null_mut();

    (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_INPUTDEVICEINFO);
    FOREIGN_CLASS_INPUTDEVICEINFO = ptr::null_mut();
}
