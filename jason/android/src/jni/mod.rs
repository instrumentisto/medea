#![allow(clippy::not_unsafe_ptr_arg_deref, clippy::missing_safety_doc)]

mod audio_track_constraints;
mod connection_handle;
mod device_video_track_constraints;
mod display_video_track_constraints;
mod input_device_info;
mod jason;
mod local_media_track;
mod media_manager_handle;
mod media_stream_settings;
mod reconnect_handle;
mod remote_media_track;
mod room_close_reason;
mod room_handle;

use std::{
    convert::TryFrom,
    ffi::{CStr, CString},
    marker::PhantomData,
    os::raw,
    ptr,
};

use jni_sys::*;

use crate::*;

#[repr(transparent)]
pub struct JForeignObjectsArray<T: ForeignClass> {
    _inner: jobjectArray,
    _marker: PhantomData<T>,
}

impl<T: ForeignClass> JForeignObjectsArray<T> {
    fn jni_invalid_value() -> Self {
        Self {
            _inner: ptr::null_mut(),
            _marker: PhantomData,
        }
    }

    fn from_jobjects(env: *mut JNIEnv, mut arr: Vec<T>) -> Self {
        let jcls: jclass = <T>::jni_class();
        assert!(!jcls.is_null());
        let arr_len = jsize::try_from(arr.len())
            .expect("invalid usize, in usize => to jsize conversation");
        let obj_arr: jobjectArray = unsafe {
            (**env).NewObjectArray.unwrap()(env, arr_len, jcls, ptr::null_mut())
        };
        assert!(!obj_arr.is_null());
        let field_id = <T>::jni_class_pointer_field();
        assert!(!field_id.is_null());
        for (i, r_obj) in arr.drain(..).enumerate() {
            let jobj: jobject =
                unsafe { (**env).AllocObject.unwrap()(env, jcls) };
            assert!(!jobj.is_null());
            let r_obj: jlong = <T>::box_object(r_obj);
            unsafe {
                (**env).SetLongField.unwrap()(env, jobj, field_id, r_obj);
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    panic!("Can not nativePtr field: catch exception");
                }
                (**env).SetObjectArrayElement.unwrap()(
                    env, obj_arr, i as jsize, jobj,
                );
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    panic!("SetObjectArrayElement({}) failed", i);
                }
                (**env).DeleteLocalRef.unwrap()(env, jobj);
            }
        }
        JForeignObjectsArray {
            _inner: obj_arr,
            _marker: PhantomData,
        }
    }
}

#[doc = " Default JNI_VERSION"]
const JNI_VERSION: jint = JNI_VERSION_1_6 as jint;

trait SwigFrom<T> {
    fn swig_from(_: T, env: *mut JNIEnv) -> Self;
}
macro_rules! swig_c_str {
    ($lit:expr) => {
        concat!($lit, "\0").as_ptr() as *const ::std::os::raw::c_char
    };
}
macro_rules! swig_assert_eq_size {
    ($x:ty, $($xs:ty), +$(,)*) => {
        $ (let _ = ::std::mem::transmute::<$x, $xs>;)+
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

pub trait ForeignClass {
    type PointedType;
    fn jni_class() -> jclass;
    fn jni_class_pointer_field() -> jfieldID;
    fn box_object(self) -> jlong;
    fn get_boxed(ptr: jlong) -> Box<Self>
    where
        Self: Sized,
    {
        let this = unsafe { jlong_to_pointer::<Self>(ptr).as_mut().unwrap() };
        unsafe { Box::from_raw(this) }
    }
    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType>;
}

pub trait ForeignEnum {
    fn as_jint(&self) -> jint;
    #[doc = " # Panics"]
    #[doc = " Panics on error"]
    fn from_jint(_: jint) -> Self;
}

pub struct JavaString {
    string: jstring,
    chars: *const raw::c_char,
    env: *mut JNIEnv,
}

impl JavaString {
    pub fn new(env: *mut JNIEnv, js: jstring) -> JavaString {
        let chars = if js.is_null() {
            ptr::null_mut()
        } else {
            unsafe {
                (**env).GetStringUTFChars.unwrap()(env, js, ptr::null_mut())
            }
        };
        JavaString {
            string: js,
            chars,
            env,
        }
    }

    pub fn to_str(&self) -> &str {
        if self.chars.is_null() {
            ""
        } else {
            let s = unsafe { CStr::from_ptr(self.chars) };
            s.to_str().unwrap()
        }
    }
}

impl Drop for JavaString {
    fn drop(&mut self) {
        assert!(!self.env.is_null());
        if !self.string.is_null() {
            assert!(!self.chars.is_null());
            unsafe {
                (**self.env).ReleaseStringUTFChars.unwrap()(
                    self.env,
                    self.string,
                    self.chars,
                )
            };
            self.env = ptr::null_mut();
            self.chars = ptr::null_mut();
        }
    }
}

struct JavaCallback {
    java_vm: *mut JavaVM,
    this: jobject,
    methods: Vec<jmethodID>,
}

impl JavaCallback {
    fn new(obj: jobject, env: *mut JNIEnv) -> JavaCallback {
        let mut java_vm: *mut JavaVM = ptr::null_mut();
        let ret = unsafe { (**env).GetJavaVM.unwrap()(env, &mut java_vm) };
        assert_eq!(0, ret, "GetJavaVm failed");
        let global_obj = unsafe { (**env).NewGlobalRef.unwrap()(env, obj) };
        assert!(!global_obj.is_null());
        JavaCallback {
            java_vm,
            this: global_obj,
            methods: Vec::new(),
        }
    }

    fn get_jni_(&self) -> JniEnvHolder {
        assert!(!self.java_vm.is_null());
        let mut env: *mut JNIEnv = ptr::null_mut();
        let res = unsafe {
            (**self.java_vm).GetEnv.unwrap()(
                self.java_vm,
                (&mut env) as *mut *mut JNIEnv as *mut *mut raw::c_void,
                JNI_VERSION,
            )
        };
        if res == (JNI_OK as jint) {
            return JniEnvHolder {
                env: Some(env),
                callback: self,
                need_detach: false,
            };
        }
        if res != (JNI_EDETACHED as jint) {
            panic!("get_jni_: GetEnv return error `{}`", res);
        }
        trait ConvertPtr<T> {
            fn convert_ptr(self) -> T;
        }
        impl ConvertPtr<*mut *mut raw::c_void> for *mut *mut JNIEnv {
            fn convert_ptr(self) -> *mut *mut raw::c_void {
                self as *mut *mut raw::c_void
            }
        }
        impl ConvertPtr<*mut *mut JNIEnv> for *mut *mut JNIEnv {
            fn convert_ptr(self) -> *mut *mut JNIEnv {
                self
            }
        }
        let res = unsafe {
            (**self.java_vm).AttachCurrentThread.unwrap()(
                self.java_vm,
                (&mut env as *mut *mut JNIEnv).convert_ptr(),
                ptr::null_mut(),
            )
        };
        if res == 0 {
            assert!(!env.is_null());
            JniEnvHolder {
                env: Some(env),
                callback: self,
                need_detach: true,
            }
        } else {
            log::error!(
                "JavaCallback::get_jnienv: AttachCurrentThread failed: {}",
                res
            );
            JniEnvHolder {
                env: None,
                callback: self,
                need_detach: false,
            }
        }
    }
}

impl Drop for JavaCallback {
    fn drop(&mut self) {
        let env = self.get_jni_();
        if let Some(env) = env.env {
            assert!(!env.is_null());
            unsafe { (**env).DeleteGlobalRef.unwrap()(env, self.this) };
        } else {
            log::error!("JavaCallback::drop failed, can not get JNIEnv");
        }
    }
}

#[doc = " According to JNI spec it should be safe to"]
#[doc = " pass pointer to JavaVm and jobject (global) across threads"]
unsafe impl Send for JavaCallback {}

struct JniEnvHolder<'a> {
    env: Option<*mut JNIEnv>,
    callback: &'a JavaCallback,
    need_detach: bool,
}

impl<'a> Drop for JniEnvHolder<'a> {
    fn drop(&mut self) {
        if self.need_detach {
            let res = unsafe {
                (**self.callback.java_vm).DetachCurrentThread.unwrap()(
                    self.callback.java_vm,
                )
            };
            if res != 0 {
                log::error!(
                    "JniEnvHolder: DetachCurrentThread failed: {}",
                    res
                );
            }
        }
    }
}

fn jni_throw(env: *mut JNIEnv, ex_class: jclass, message: &str) {
    let c_message = CString::new(message).unwrap();
    let res =
        unsafe { (**env).ThrowNew.unwrap()(env, ex_class, c_message.as_ptr()) };
    if res != 0 {
        log::error!(
            "JNI ThrowNew({}) failed for class {:?} failed",
            message,
            ex_class
        );
    }
}

fn jni_throw_exception(env: *mut JNIEnv, message: &str) {
    let exception_class = unsafe { JAVA_LANG_EXCEPTION };
    jni_throw(env, exception_class, message)
}

fn object_to_jobject<T: ForeignClass>(
    env: *mut JNIEnv,
    rust_obj: T,
) -> jobject {
    let jcls = <T>::jni_class();
    assert!(!jcls.is_null());
    let field_id = <T>::jni_class_pointer_field();
    assert!(!field_id.is_null());
    let jobj: jobject = unsafe { (**env).AllocObject.unwrap()(env, jcls) };
    assert!(!jobj.is_null(), "object_to_jobject: AllocObject failed");
    let ret: jlong = <T>::box_object(rust_obj);
    unsafe {
        (**env).SetLongField.unwrap()(env, jobj, field_id, ret);
        if (**env).ExceptionCheck.unwrap()(env) != 0 {
            panic!(
                "object_to_jobject: Can not set nativePtr field: catch \
                 exception"
            );
        }
    }
    jobj
}

fn from_std_string_jstring(x: String, env: *mut JNIEnv) -> jstring {
    let x = x.into_bytes();
    unsafe {
        let x = CString::from_vec_unchecked(x);
        (**env).NewStringUTF.unwrap()(env, x.as_ptr())
    }
}

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

impl SwigFrom<FacingMode> for jobject {
    fn swig_from(x: FacingMode, env: *mut JNIEnv) -> jobject {
        let cls = unsafe { FOREIGN_ENUM_FACINGMODE };
        assert!(!cls.is_null());
        let static_field_id: jfieldID = match x {
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
        assert!(!static_field_id.is_null());
        let ret: jobject = unsafe {
            (**env).GetStaticObjectField.unwrap()(env, cls, static_field_id)
        };
        assert!(
            !ret.is_null(),
            concat!("Can get value of item in ", "com/jason/api/FacingMode")
        );
        ret
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

impl SwigFrom<MediaKind> for jobject {
    fn swig_from(x: MediaKind, env: *mut JNIEnv) -> jobject {
        let cls = unsafe { FOREIGN_ENUM_MEDIAKIND };
        assert!(!cls.is_null());
        let static_field_id: jfieldID = match x {
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
        assert!(!static_field_id.is_null());
        let ret: jobject = unsafe {
            (**env).GetStaticObjectField.unwrap()(env, cls, static_field_id)
        };
        assert!(
            !ret.is_null(),
            concat!("Can get value of item in ", "com/jason/api/MediaKind")
        );
        ret
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

impl SwigFrom<MediaSourceKind> for jobject {
    fn swig_from(x: MediaSourceKind, env: *mut JNIEnv) -> jobject {
        let cls = unsafe { FOREIGN_ENUM_MEDIASOURCEKIND };
        assert!(!cls.is_null());
        let static_field_id: jfieldID = match x {
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
        assert!(!static_field_id.is_null());
        let ret: jobject = unsafe {
            (**env).GetStaticObjectField.unwrap()(env, cls, static_field_id)
        };
        assert!(
            !ret.is_null(),
            concat!(
                "Can get value of item in ",
                "com/jason/api/MediaSourceKind"
            )
        );
        ret
    }
}

impl<T: ForeignClass> Consumer<T> for JavaCallback {
    fn accept(&self, arg: T) {
        swig_assert_eq_size!(raw::c_uint, u32);
        swig_assert_eq_size!(raw::c_int, i32);
        if let Some(env) = self.get_jni_().env {
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0],
                    object_to_jobject(env, arg),
                );
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(
                        stringify!(accept),
                        ": java throw exception"
                    ));
                    (**env).ExceptionDescribe.unwrap()(env);
                    (**env).ExceptionClear.unwrap()(env);
                }
            };
        }
    }
}

impl Consumer<()> for JavaCallback {
    fn accept(&self, _: ()) {
        swig_assert_eq_size!(raw::c_uint, u32);
        swig_assert_eq_size!(raw::c_int, i32);
        if let Some(env) = self.get_jni_().env {
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0],
                    ptr::null::<jobject>(),
                );
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(
                        stringify!(accept),
                        ": java throw exception"
                    ));
                    (**env).ExceptionDescribe.unwrap()(env);
                    (**env).ExceptionClear.unwrap()(env);
                }
            };
        }
    }
}

impl Consumer<u8> for JavaCallback {
    fn accept(&self, arg: u8) {
        swig_assert_eq_size!(raw::c_uint, u32);
        swig_assert_eq_size!(raw::c_int, i32);
        if let Some(env) = self.get_jni_().env {
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0],
                    i32::from(jshort::from(arg)),
                );
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(
                        stringify!(accept),
                        ": java throw exception"
                    ));
                    (**env).ExceptionDescribe.unwrap()(env);
                    (**env).ExceptionClear.unwrap()(env);
                }
            };
        }
    }
}

impl SwigFrom<jobject> for Box<dyn Consumer<()>> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for ConsumerRemoteMediaTrack"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("accept"),
                swig_c_str!("(Ljava/lang/Object;)V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

#[doc = ""]
impl SwigFrom<jobject> for Box<dyn Consumer<RemoteMediaTrack>> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for ConsumerRemoteMediaTrack"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("accept"),
                swig_c_str!("(Ljava/lang/Object;)V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

#[doc = ""]
impl SwigFrom<jobject> for Box<dyn Consumer<u8>> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for ConsumerShort"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("accept"),
                swig_c_str!("(Ljava/lang/Object;)V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

impl ForeignClass for JasonError {
    type PointedType = JasonError;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_JASONERROR }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_JASONERROR_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let this = unsafe { jlong_to_pointer::<Self>(x).as_mut().unwrap() };
        ptr::NonNull::<Self::PointedType>::new(this).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeName(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &JasonError =
        unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
    let ret: String = JasonError::name(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeMessage(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &JasonError =
        unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
    let ret: String = JasonError::message(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeTrace(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &JasonError =
        unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
    let ret: String = JasonError::trace(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    JasonError::get_boxed(ptr);
}

impl SwigFrom<jobject> for Box<dyn Consumer<ConnectionHandle>> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for ConsumerConnectionHandle"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("accept"),
                swig_c_str!("(Ljava/lang/Object;)V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

#[doc = ""]
impl SwigFrom<jobject> for Box<dyn Consumer<RoomCloseReason>> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for ConsumerRoomCloseReason"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("accept"),
                swig_c_str!("(Ljava/lang/Object;)V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

#[doc = ""]
impl SwigFrom<jobject> for Box<dyn Consumer<LocalMediaTrack>> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for ConsumerLocalMediaTrack"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("accept"),
                swig_c_str!("(Ljava/lang/Object;)V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

#[doc = ""]
impl SwigFrom<jobject> for Box<dyn Consumer<JasonError>> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for ConsumerJasonError"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("accept"),
                swig_c_str!("(Ljava/lang/Object;)V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

#[doc = ""]
impl SwigFrom<jobject> for Box<dyn Consumer<ReconnectHandle>> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for ConsumerReconnectHandle"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("accept"),
                swig_c_str!("(Ljava/lang/Object;)V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

impl ForeignClass for ConstraintsUpdateException {
    type PointedType = ConstraintsUpdateException;

    fn jni_class() -> jclass {
        unsafe { FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION }
    }

    fn jni_class_pointer_field() -> jfieldID {
        unsafe { FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_NATIVEPTR_FIELD }
    }

    fn box_object(self) -> jlong {
        Box::into_raw(Box::new(self)) as i64
    }

    fn get_ptr(x: jlong) -> ptr::NonNull<Self::PointedType> {
        let x = unsafe { jlong_to_pointer::<Self>(x).as_mut().unwrap() };
        ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeName(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &ConstraintsUpdateException = unsafe {
        jlong_to_pointer::<ConstraintsUpdateException>(this)
            .as_mut()
            .unwrap()
    };
    let ret: String = ConstraintsUpdateException::name(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeRecoverReason(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let this: &ConstraintsUpdateException = unsafe {
        jlong_to_pointer::<ConstraintsUpdateException>(this)
            .as_mut()
            .unwrap()
    };
    let ret: Option<JasonError> =
        ConstraintsUpdateException::recover_reason(this);
    let ret: jlong = match ret {
        Some(x) => {
            let ptr = <JasonError>::box_object(x);
            debug_assert_ne!(0, ptr);
            ptr
        }
        None => 0,
    };
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeRecoverFailReason(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let this: &ConstraintsUpdateException = unsafe {
        jlong_to_pointer::<ConstraintsUpdateException>(this)
            .as_mut()
            .unwrap()
    };
    let ret: Option<JasonError> =
        ConstraintsUpdateException::recover_fail_reasons(this);
    let ret: jlong = match ret {
        Some(x) => {
            let ptr = <JasonError>::box_object(x);
            debug_assert_ne!(0, ptr);
            ptr
        }
        None => 0,
    };
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeError(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let this: &ConstraintsUpdateException = unsafe {
        jlong_to_pointer::<ConstraintsUpdateException>(this)
            .as_mut()
            .unwrap()
    };
    let ret: Option<JasonError> = ConstraintsUpdateException::error(this);
    let ret: jlong = match ret {
        Some(x) => {
            let ptr = <JasonError>::box_object(x);
            debug_assert_ne!(0, ptr);
            ptr
        }
        None => 0,
    };
    ret
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_ConstraintsUpdateException_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    ptr: jlong,
) {
    ConstraintsUpdateException::get_boxed(ptr);
}

macro_rules! cache_foreign_class {
    ($env:ident; $class:expr => $path:expr) => {
            let class_local_ref = (**$env).FindClass.unwrap()(
                $env,
                swig_c_str!($path),
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
                    swig_c_str!($field_name),
                    swig_c_str!($field_sig),
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
    java_vm: *mut JavaVM,
    _reserved: *mut raw::c_void,
) -> jint {
    // TODO: dont panic, return log and return JNI_ERR.

    let mut env: *mut JNIEnv = ptr::null_mut();
    let res = (**java_vm).GetEnv.unwrap()(
        java_vm,
        (&mut env) as *mut *mut JNIEnv as *mut *mut raw::c_void,
        JNI_VERSION,
    );
    if res != (JNI_OK as jint) {
        panic!("JNI GetEnv in JNI_OnLoad failed, return code {}", res);
    }
    assert!(!env.is_null());

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Exception"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/Exception"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/Exception");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_EXCEPTION = class;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Long"));
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
        swig_c_str!("longValue"),
        swig_c_str!("()J"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Long method longValue sig ()J failed"
    );
    JAVA_LANG_LONG_LONG_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Byte"));
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
        swig_c_str!("byteValue"),
        swig_c_str!("()B"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Byte method byteValue sig ()B failed"
    );
    JAVA_LANG_BYTE_BYTE_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Integer"));
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
        swig_c_str!("intValue"),
        swig_c_str!("()I"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Integer method intValue sig ()I \
         failed"
    );
    JAVA_LANG_INTEGER_INT_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Short"));
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
        swig_c_str!("shortValue"),
        swig_c_str!("()S"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Short method shortValue sig ()S \
         failed"
    );
    JAVA_LANG_SHORT_SHORT_VALUE = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Double"));
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
        swig_c_str!("doubleValue"),
        swig_c_str!("()D"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Double method doubleValue sig ()D \
         failed"
    );
    JAVA_LANG_DOUBLE_DOUBLE_VALUE_METHOD = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/String"));
    assert!(
        !class_local_ref.is_null(),
        "FindClass failed for java/lang/String"
    );
    let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
    assert!(!class.is_null(), "FindClass failed for java/lang/String");
    (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
    JAVA_LANG_STRING = class;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Float"));
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
        swig_c_str!("floatValue"),
        swig_c_str!("()F"),
    );
    assert!(
        !method_id.is_null(),
        "GetMethodID for class java/lang/Float method floatValue sig ()F \
         failed"
    );
    JAVA_LANG_FLOAT_FLOAT_VALUE = method_id;

    let class_local_ref = (**env).FindClass.unwrap()(
        env,
        swig_c_str!("java/util/OptionalDouble"),
    );
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
        swig_c_str!("of"),
        swig_c_str!("(D)Ljava/util/OptionalDouble;"),
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
        swig_c_str!("empty"),
        swig_c_str!("()Ljava/util/OptionalDouble;"),
    );
    assert!(
        !method_id.is_null(),
        "GetStaticMethodID for class java/util/OptionalDouble method empty \
         sig ()Ljava/util/OptionalDouble; failed"
    );
    JAVA_UTIL_OPTIONAL_DOUBLE_EMPTY = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/util/OptionalInt"));
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
        swig_c_str!("of"),
        swig_c_str!("(I)Ljava/util/OptionalInt;"),
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
        swig_c_str!("empty"),
        swig_c_str!("()Ljava/util/OptionalInt;"),
    );
    assert!(
        !method_id.is_null(),
        "GetStaticMethodID for class java/util/OptionalInt method empty sig \
         ()Ljava/util/OptionalInt; failed"
    );
    JAVA_UTIL_OPTIONAL_INT_EMPTY = method_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("java/util/OptionalLong"));
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
        swig_c_str!("of"),
        swig_c_str!("(J)Ljava/util/OptionalLong;"),
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
        swig_c_str!("empty"),
        swig_c_str!("()Ljava/util/OptionalLong;"),
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

    let class_local_ref = (**env).FindClass.unwrap()(
        env,
        swig_c_str!("com/jason/api/FacingMode"),
    );
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
    let field_id: jfieldID = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        swig_c_str!("User"),
        swig_c_str!("Lcom/jason/api/FacingMode;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/FacingMode method User sig \
         Lcom/jason/api/FacingMode; failed"
    );
    FOREIGN_ENUM_FACINGMODE_USER = field_id;
    let field_id: jfieldID = (**env).GetStaticFieldID.unwrap()(
        env,
        class,
        swig_c_str!("Environment"),
        swig_c_str!("Lcom/jason/api/FacingMode;"),
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
        swig_c_str!("Left"),
        swig_c_str!("Lcom/jason/api/FacingMode;"),
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
        swig_c_str!("Right"),
        swig_c_str!("Lcom/jason/api/FacingMode;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/FacingMode method Right sig \
         Lcom/jason/api/FacingMode; failed"
    );
    FOREIGN_ENUM_FACINGMODE_RIGHT = field_id;

    let class_local_ref =
        (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/MediaKind"));
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
        swig_c_str!("Audio"),
        swig_c_str!("Lcom/jason/api/MediaKind;"),
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
        swig_c_str!("Video"),
        swig_c_str!("Lcom/jason/api/MediaKind;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/MediaKind method Video sig \
         Lcom/jason/api/MediaKind; failed"
    );
    FOREIGN_ENUM_MEDIAKIND_VIDEO = field_id;

    let class_local_ref = (**env).FindClass.unwrap()(
        env,
        swig_c_str!("com/jason/api/MediaSourceKind"),
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
        swig_c_str!("Device"),
        swig_c_str!("Lcom/jason/api/MediaSourceKind;"),
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
        swig_c_str!("Display"),
        swig_c_str!("Lcom/jason/api/MediaSourceKind;"),
    );
    assert!(
        !field_id.is_null(),
        "GetStaticFieldID for class com/jason/api/MediaSourceKind method \
         Display sig Lcom/jason/api/MediaSourceKind; failed"
    );
    FOREIGN_ENUM_MEDIASOURCEKIND_DISPLAY = field_id;

    JNI_VERSION
}

#[no_mangle]
pub unsafe extern "system" fn JNI_OnUnload(
    java_vm: *mut JavaVM,
    _reserved: *mut raw::c_void,
) {
    assert!(!java_vm.is_null());
    let mut env: *mut JNIEnv = ptr::null_mut();
    let res = (**java_vm).GetEnv.unwrap()(
        java_vm,
        (&mut env) as *mut *mut JNIEnv as *mut *mut raw::c_void,
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
