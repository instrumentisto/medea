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
mod remoted_media_track;
mod room_close_reason;
mod room_handle;

use jni_sys::*;

use crate::*;

#[repr(transparent)]
pub struct JForeignObjectsArray<T: ForeignClass> {
    inner: jobjectArray,
    _marker: ::std::marker::PhantomData<T>,
}

#[doc = " Default JNI_VERSION"]
const JNI_VERSION: jint = JNI_VERSION_1_6 as jint;

#[doc = ""]
trait SwigInto<T> {
    fn swig_into(self, env: *mut JNIEnv) -> T;
}

#[doc = ""]
trait SwigFrom<T> {
    fn swig_from(_: T, env: *mut JNIEnv) -> Self;
}
macro_rules! swig_c_str {
    ($ lit : expr) => {
        concat!($lit, "\0").as_ptr() as *const ::std::os::raw::c_char
    };
}
macro_rules! swig_assert_eq_size { ($ x : ty , $ ($ xs : ty) ,+ $ (,) *) => { $ (let _ = :: std :: mem :: transmute ::<$ x , $ xs >;) + } ; }
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
    fn box_object(x: Self) -> jlong;
    fn unbox_object(x: jlong) -> Self;
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType>;
}

pub trait ForeignEnum {
    fn as_jint(&self) -> jint;
    #[doc = " # Panics"]
    #[doc = " Panics on error"]
    fn from_jint(_: jint) -> Self;
}

pub struct JavaString {
    string: jstring,
    chars: *const ::std::os::raw::c_char,
    env: *mut JNIEnv,
}

impl JavaString {
    pub fn new(env: *mut JNIEnv, js: jstring) -> JavaString {
        let chars = if !js.is_null() {
            unsafe {
                (**env).GetStringUTFChars.unwrap()(
                    env,
                    js,
                    ::std::ptr::null_mut(),
                )
            }
        } else {
            ::std::ptr::null_mut()
        };
        JavaString {
            string: js,
            chars,
            env,
        }
    }

    pub fn to_str(&self) -> &str {
        if !self.chars.is_null() {
            let s = unsafe { ::std::ffi::CStr::from_ptr(self.chars) };
            s.to_str().unwrap()
        } else {
            ""
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
            self.env = ::std::ptr::null_mut();
            self.chars = ::std::ptr::null_mut();
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
        let mut java_vm: *mut JavaVM = ::std::ptr::null_mut();
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
        let mut env: *mut JNIEnv = ::std::ptr::null_mut();
        let res = unsafe {
            (**self.java_vm).GetEnv.unwrap()(
                self.java_vm,
                (&mut env) as *mut *mut JNIEnv
                    as *mut *mut ::std::os::raw::c_void,
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
        impl ConvertPtr<*mut *mut ::std::os::raw::c_void> for *mut *mut JNIEnv {
            fn convert_ptr(self) -> *mut *mut ::std::os::raw::c_void {
                self as *mut *mut ::std::os::raw::c_void
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
                ::std::ptr::null_mut(),
            )
        };
        if res != 0 {
            log::error!(
                "JavaCallback::get_jnienv: AttachCurrentThread failed: {}",
                res
            );
            JniEnvHolder {
                env: None,
                callback: self,
                need_detach: false,
            }
        } else {
            assert!(!env.is_null());
            JniEnvHolder {
                env: Some(env),
                callback: self,
                need_detach: true,
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
    let c_message = ::std::ffi::CString::new(message).unwrap();
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

fn object_to_jobject<T: ForeignClass>(env: *mut JNIEnv, obj: T) -> jobject {
    let jcls = <T>::jni_class();
    assert!(!jcls.is_null());
    let field_id = <T>::jni_class_pointer_field();
    assert!(!field_id.is_null());
    let jobj: jobject = unsafe { (**env).AllocObject.unwrap()(env, jcls) };
    assert!(!jobj.is_null(), "object_to_jobject: AllocObject failed");
    let ret: jlong = <T>::box_object(obj);
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

fn vec_of_objects_to_jobject_array<T: ForeignClass>(
    env: *mut JNIEnv,
    mut arr: Vec<T>,
) -> JForeignObjectsArray<T> {
    let jcls: jclass = <T>::jni_class();
    assert!(!jcls.is_null());
    let arr_len =
        <jsize as ::std::convert::TryFrom<usize>>::try_from(arr.len())
            .expect("invalid usize, in usize => to jsize conversation");
    let obj_arr: jobjectArray = unsafe {
        (**env).NewObjectArray.unwrap()(
            env,
            arr_len,
            jcls,
            ::std::ptr::null_mut(),
        )
    };
    assert!(!obj_arr.is_null());
    let field_id = <T>::jni_class_pointer_field();
    assert!(!field_id.is_null());
    for (i, r_obj) in arr.drain(..).enumerate() {
        let jobj: jobject = unsafe { (**env).AllocObject.unwrap()(env, jcls) };
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
        inner: obj_arr,
        _marker: ::std::marker::PhantomData,
    }
}

trait JniInvalidValue {
    fn jni_invalid_value() -> Self;
}

impl<T> JniInvalidValue for *const T {
    fn jni_invalid_value() -> Self {
        ::std::ptr::null()
    }
}

impl<T> JniInvalidValue for *mut T {
    fn jni_invalid_value() -> Self {
        ::std::ptr::null_mut()
    }
}

impl JniInvalidValue for () {
    fn jni_invalid_value() {}
}

impl<T: ForeignClass> JniInvalidValue for JForeignObjectsArray<T> {
    fn jni_invalid_value() -> Self {
        Self {
            inner: ::std::ptr::null_mut(),
            _marker: ::std::marker::PhantomData,
        }
    }
}

macro_rules! impl_jni_jni_invalid_value { ($ ($ type : ty) *) => ($ (impl JniInvalidValue for $ type { fn jni_invalid_value () -> Self { <$ type >:: default () } }) *) }
impl_jni_jni_invalid_value! { jbyte jshort jint jlong jfloat jdouble }

pub fn u64_to_jlong_checked(x: u64) -> jlong {
    <jlong as ::std::convert::TryFrom<u64>>::try_from(x)
        .expect("invalid u64, in u64 => jlong conversation")
}

fn from_std_string_jstring(x: String, env: *mut JNIEnv) -> jstring {
    let x = x.into_bytes();
    unsafe {
        let x = ::std::ffi::CString::from_vec_unchecked(x);
        (**env).NewStringUTF.unwrap()(env, x.as_ptr())
    }
}

macro_rules! define_array_handling_code { ($ ([jni_arr_type = $ jni_arr_type : ident , rust_arr_wrapper = $ rust_arr_wrapper : ident , jni_get_array_elements = $ jni_get_array_elements : ident , jni_elem_type = $ jni_elem_type : ident , rust_elem_type = $ rust_elem_type : ident , jni_release_array_elements = $ jni_release_array_elements : ident , jni_new_array = $ jni_new_array : ident , jni_set_array_region = $ jni_set_array_region : ident]) ,*) => { $ (# [allow (dead_code)] struct $ rust_arr_wrapper { array : $ jni_arr_type , data : * mut $ jni_elem_type , env : * mut JNIEnv , } # [allow (dead_code)] impl $ rust_arr_wrapper { fn new (env : * mut JNIEnv , array : $ jni_arr_type) -> $ rust_arr_wrapper { assert ! (! array . is_null ()) ; let data = unsafe { (** env) .$ jni_get_array_elements . unwrap () (env , array , :: std :: ptr :: null_mut ()) } ; $ rust_arr_wrapper { array , data , env } } fn to_slice (& self) -> & [$ rust_elem_type] { unsafe { let len : jsize = (** self . env) . GetArrayLength . unwrap () (self . env , self . array) ; assert ! ((len as u64) <= (usize :: max_value () as u64)) ; :: std :: slice :: from_raw_parts (self . data , len as usize) } } fn from_slice_to_raw (arr : & [$ rust_elem_type] , env : * mut JNIEnv) -> $ jni_arr_type { assert ! ((arr . len () as u64) <= (jsize :: max_value () as u64)) ; let jarr : $ jni_arr_type = unsafe { (** env) .$ jni_new_array . unwrap () (env , arr . len () as jsize) } ; assert ! (! jarr . is_null ()) ; unsafe { (** env) .$ jni_set_array_region . unwrap () (env , jarr , 0 , arr . len () as jsize , arr . as_ptr ()) ; if (** env) . ExceptionCheck . unwrap () (env) != 0 { panic ! ("{}:{} {} failed" , file ! () , line ! () , stringify ! ($ jni_set_array_region)) ; } } jarr } } # [allow (dead_code)] impl Drop for $ rust_arr_wrapper { fn drop (& mut self) { assert ! (! self . env . is_null ()) ; assert ! (! self . array . is_null ()) ; unsafe { (** self . env) .$ jni_release_array_elements . unwrap () (self . env , self . array , self . data , JNI_ABORT as jint ,) } ; } }) * } }

define_array_handling_code!(
    [
        jni_arr_type = jbyteArray,
        rust_arr_wrapper = JavaByteArray,
        jni_get_array_elements = GetByteArrayElements,
        jni_elem_type = jbyte,
        rust_elem_type = i8,
        jni_release_array_elements = ReleaseByteArrayElements,
        jni_new_array = NewByteArray,
        jni_set_array_region = SetByteArrayRegion
    ],
    [
        jni_arr_type = jshortArray,
        rust_arr_wrapper = JavaShortArray,
        jni_get_array_elements = GetShortArrayElements,
        jni_elem_type = jshort,
        rust_elem_type = i16,
        jni_release_array_elements = ReleaseShortArrayElements,
        jni_new_array = NewShortArray,
        jni_set_array_region = SetShortArrayRegion
    ],
    [
        jni_arr_type = jintArray,
        rust_arr_wrapper = JavaIntArray,
        jni_get_array_elements = GetIntArrayElements,
        jni_elem_type = jint,
        rust_elem_type = i32,
        jni_release_array_elements = ReleaseIntArrayElements,
        jni_new_array = NewIntArray,
        jni_set_array_region = SetIntArrayRegion
    ],
    [
        jni_arr_type = jlongArray,
        rust_arr_wrapper = JavaLongArray,
        jni_get_array_elements = GetLongArrayElements,
        jni_elem_type = jlong,
        rust_elem_type = i64,
        jni_release_array_elements = ReleaseLongArrayElements,
        jni_new_array = NewLongArray,
        jni_set_array_region = SetLongArrayRegion
    ],
    [
        jni_arr_type = jfloatArray,
        rust_arr_wrapper = JavaFloatArray,
        jni_get_array_elements = GetFloatArrayElements,
        jni_elem_type = jfloat,
        rust_elem_type = f32,
        jni_release_array_elements = ReleaseFloatArrayElements,
        jni_new_array = NewFloatArray,
        jni_set_array_region = SetFloatArrayRegion
    ],
    [
        jni_arr_type = jdoubleArray,
        rust_arr_wrapper = JavaDoubleArray,
        jni_get_array_elements = GetDoubleArrayElements,
        jni_elem_type = jdouble,
        rust_elem_type = f64,
        jni_release_array_elements = ReleaseDoubleArrayElements,
        jni_new_array = NewDoubleArray,
        jni_set_array_region = SetDoubleArrayRegion
    ]
);

impl ForeignEnum for FacingMode {
    fn as_jint(&self) -> jint {
        match *self {
            FacingMode::User => 0i32,
            FacingMode::Environment => 1i32,
            FacingMode::Left => 2i32,
            FacingMode::Right => 3i32,
        }
    }

    fn from_jint(x: jint) -> Self {
        match x {
            0i32 => FacingMode::User,
            1i32 => FacingMode::Environment,
            2i32 => FacingMode::Left,
            3i32 => FacingMode::Right,
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
            MediaKind::Audio => 0i32,
            MediaKind::Video => 1i32,
        }
    }

    fn from_jint(x: jint) -> Self {
        match x {
            0i32 => MediaKind::Audio,
            1i32 => MediaKind::Video,
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
            MediaSourceKind::Device => 0i32,
            MediaSourceKind::Display => 1i32,
        }
    }

    fn from_jint(x: jint) -> Self {
        match x {
            0i32 => MediaSourceKind::Device,
            1i32 => MediaSourceKind::Display,
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

impl Callback for JavaCallback {
    fn call(&self) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0usize],
                );
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(
                        stringify!(call),
                        ": java throw exception"
                    ));
                    (**env).ExceptionDescribe.unwrap()(env);
                    (**env).ExceptionClear.unwrap()(env);
                }
            };
        }
    }
}

#[doc = ""]
impl SwigFrom<jobject> for Box<dyn Callback> {
    fn swig_from(this: jobject, env: *mut JNIEnv) -> Self {
        let mut cb = JavaCallback::new(this, env);
        cb.methods.reserve(1);
        let class = unsafe { (**env).GetObjectClass.unwrap()(env, cb.this) };
        assert!(
            !class.is_null(),
            "GetObjectClass return null class for Callback"
        );
        let method_id: jmethodID = unsafe {
            (**env).GetMethodID.unwrap()(
                env,
                class,
                swig_c_str!("call"),
                swig_c_str!("()V"),
            )
        };
        assert!(!method_id.is_null(), "Can not find call id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}

impl Consumer<RemoteMediaTrack> for JavaCallback {
    fn accept(&self, a0: RemoteMediaTrack) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            let a0: jobject = object_to_jobject(env, a0);
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0usize],
                    a0,
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
    fn accept(&self, a0: u8) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            let a0: jshort = jshort::from(a0);
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0usize],
                    a0 as ::std::os::raw::c_int,
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
                swig_c_str!("(Lcom/jason/api/RemoteMediaTrack;)V"),
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
                swig_c_str!("(S)V"),
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

    fn box_object(this: Self) -> jlong {
        let this: Box<JasonError> = Box::new(this);
        let this: *mut JasonError = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut JasonError =
            unsafe { jlong_to_pointer::<JasonError>(x).as_mut().unwrap() };
        let x: Box<JasonError> = unsafe { Box::from_raw(x) };
        let x: JasonError = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut JasonError =
            unsafe { jlong_to_pointer::<JasonError>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
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
    this: jlong,
) {
    let this: *mut JasonError =
        unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
    let this: Box<JasonError> = unsafe { Box::from_raw(this) };
    drop(this);
}

impl Consumer<ConnectionHandle> for JavaCallback {
    fn accept(&self, a0: ConnectionHandle) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            let a0: jobject = object_to_jobject(env, a0);
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0usize],
                    a0,
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

impl Consumer<RoomCloseReason> for JavaCallback {
    fn accept(&self, a0: RoomCloseReason) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            let a0: jobject = object_to_jobject(env, a0);
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0usize],
                    a0,
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

impl Consumer<LocalMediaTrack> for JavaCallback {
    fn accept(&self, a0: LocalMediaTrack) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            let a0: jobject = object_to_jobject(env, a0);
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0usize],
                    a0,
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

impl Consumer<JasonError> for JavaCallback {
    fn accept(&self, a0: JasonError) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            let a0: jobject = object_to_jobject(env, a0);
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0usize],
                    a0,
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

impl Consumer<ReconnectHandle> for JavaCallback {
    fn accept(&self, a0: ReconnectHandle) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            let a0: jobject = object_to_jobject(env, a0);
            unsafe {
                (**env).CallVoidMethod.unwrap()(
                    env,
                    self.this,
                    self.methods[0usize],
                    a0,
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

#[doc = ""]
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
                swig_c_str!("(Lcom/jason/api/ConnectionHandle;)V"),
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
                swig_c_str!("(Lcom/jason/api/RoomCloseReason;)V"),
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
                swig_c_str!("(Lcom/jason/api/LocalMediaTrack;)V"),
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
                swig_c_str!("(Lcom/jason/api/JasonError;)V"),
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
                swig_c_str!("(Lcom/jason/api/ReconnectHandle;)V"),
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

    fn box_object(this: Self) -> jlong {
        let this: Box<ConstraintsUpdateException> = Box::new(this);
        let this: *mut ConstraintsUpdateException = Box::into_raw(this);
        this as jlong
    }

    fn unbox_object(x: jlong) -> Self {
        let x: *mut ConstraintsUpdateException = unsafe {
            jlong_to_pointer::<ConstraintsUpdateException>(x)
                .as_mut()
                .unwrap()
        };
        let x: Box<ConstraintsUpdateException> = unsafe { Box::from_raw(x) };
        let x: ConstraintsUpdateException = *x;
        x
    }

    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut ConstraintsUpdateException = unsafe {
            jlong_to_pointer::<ConstraintsUpdateException>(x)
                .as_mut()
                .unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
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
    this: jlong,
) {
    let this: *mut ConstraintsUpdateException = unsafe {
        jlong_to_pointer::<ConstraintsUpdateException>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<ConstraintsUpdateException> = unsafe { Box::from_raw(this) };
    drop(this);
}

static mut JAVA_UTIL_OPTIONAL_INT: jclass = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_INT_OF: jmethodID = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_INT_EMPTY: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION: jclass =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_REMOTEMEDIATRACK: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_REMOTEMEDIATRACK_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_JASONERROR: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_JASONERROR_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE: jclass = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE_OF: jmethodID = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE_EMPTY: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_MEDIAMANAGERHANDLE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_MEDIAMANAGERHANDLE_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_ROOMCLOSEREASON: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_ROOMCLOSEREASON_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_RECONNECTHANDLE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_RECONNECTHANDLE_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut JAVA_LANG_SHORT: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_SHORT_SHORT_VALUE: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_MEDIASTREAMSETTINGS: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_MEDIASTREAMSETTINGS_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut JAVA_LANG_DOUBLE: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_DOUBLE_DOUBLE_VALUE_METHOD: jmethodID =
    ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_USER: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_IRONMENT: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_LEFT: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_RIGHT: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_LOCALMEDIATRACK: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_LOCALMEDIATRACK_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS: jclass =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_JASON: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_JASON_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut JAVA_LANG_BYTE: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_BYTE_BYTE_VALUE: jmethodID = ::std::ptr::null_mut();
static mut JAVA_LANG_INTEGER: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_INTEGER_INT_VALUE: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_CONNECTIONHANDLE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG: jclass = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG_OF: jmethodID = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG_EMPTY: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND: jclass = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND_AUDIO: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND_VIDEO: jfieldID = ::std::ptr::null_mut();
static mut JAVA_LANG_LONG: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_LONG_LONG_VALUE: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS: jclass =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD:
    jfieldID = ::std::ptr::null_mut();
static mut JAVA_LANG_EXCEPTION: jclass = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND: jclass = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND_DEVICE: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND_DISPLAY: jfieldID =
    ::std::ptr::null_mut();
static mut JAVA_LANG_FLOAT: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_FLOAT_FLOAT_VALUE: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_ROOMHANDLE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_ROOMHANDLE_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_INPUTDEVICEINFO: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_INPUTDEVICEINFO_NATIVEPTR_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut JAVA_LANG_STRING: jclass = ::std::ptr::null_mut();

#[no_mangle]
pub extern "system" fn JNI_OnLoad(
    java_vm: *mut JavaVM,
    _reserved: *mut ::std::os::raw::c_void,
) -> jint {
    assert!(!java_vm.is_null());
    let mut env: *mut JNIEnv = ::std::ptr::null_mut();
    let res = unsafe {
        (**java_vm).GetEnv.unwrap()(
            java_vm,
            (&mut env) as *mut *mut JNIEnv as *mut *mut ::std::os::raw::c_void,
            JNI_VERSION,
        )
    };
    if res != (JNI_OK as jint) {
        panic!("JNI GetEnv in JNI_OnLoad failed, return code {}", res);
    }
    assert!(!env.is_null());
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("java/util/OptionalInt"),
        );
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
            "GetStaticMethodID for class java/util/OptionalInt method empty \
             sig ()Ljava/util/OptionalInt; failed"
        );
        JAVA_UTIL_OPTIONAL_INT_EMPTY = method_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/ConstraintsUpdateException"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/ConstraintsUpdateException"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/ConstraintsUpdateException"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class \
             com/jason/api/ConstraintsUpdateException method nativePtr sig J \
             failed"
        );
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/RemoteMediaTrack"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/RemoteMediaTrack"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/RemoteMediaTrack"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_REMOTEMEDIATRACK = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/RemoteMediaTrack method \
             nativePtr sig J failed"
        );
        FOREIGN_CLASS_REMOTEMEDIATRACK_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/JasonError"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/JasonError"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/JasonError"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_JASONERROR = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/JasonError method \
             nativePtr sig J failed"
        );
        FOREIGN_CLASS_JASONERROR_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
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
            "GetStaticMethodID for class java/util/OptionalDouble method of \
             sig (D)Ljava/util/OptionalDouble; failed"
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
            "GetStaticMethodID for class java/util/OptionalDouble method \
             empty sig ()Ljava/util/OptionalDouble; failed"
        );
        JAVA_UTIL_OPTIONAL_DOUBLE_EMPTY = method_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/MediaManagerHandle"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/MediaManagerHandle"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/MediaManagerHandle"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_MEDIAMANAGERHANDLE = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/MediaManagerHandle \
             method nativePtr sig J failed"
        );
        FOREIGN_CLASS_MEDIAMANAGERHANDLE_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/RoomCloseReason"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/RoomCloseReason"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/RoomCloseReason"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_ROOMCLOSEREASON = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/RoomCloseReason method \
             nativePtr sig J failed"
        );
        FOREIGN_CLASS_ROOMCLOSEREASON_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/ReconnectHandle"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/ReconnectHandle"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/ReconnectHandle"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_RECONNECTHANDLE = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/ReconnectHandle method \
             nativePtr sig J failed"
        );
        FOREIGN_CLASS_RECONNECTHANDLE_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
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
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/MediaStreamSettings"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/MediaStreamSettings"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/MediaStreamSettings"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_MEDIASTREAMSETTINGS = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/MediaStreamSettings \
             method nativePtr sig J failed"
        );
        FOREIGN_CLASS_MEDIASTREAMSETTINGS_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
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
            "GetMethodID for class java/lang/Double method doubleValue sig \
             ()D failed"
        );
        JAVA_LANG_DOUBLE_DOUBLE_VALUE_METHOD = method_id;
    }
    unsafe {
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
            "GetStaticFieldID for class com/jason/api/FacingMode method User \
             sig Lcom/jason/api/FacingMode; failed"
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
            "GetStaticFieldID for class com/jason/api/FacingMode method Left \
             sig Lcom/jason/api/FacingMode; failed"
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
            "GetStaticFieldID for class com/jason/api/FacingMode method Right \
             sig Lcom/jason/api/FacingMode; failed"
        );
        FOREIGN_ENUM_FACINGMODE_RIGHT = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/LocalMediaTrack"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/LocalMediaTrack"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/LocalMediaTrack"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_LOCALMEDIATRACK = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/LocalMediaTrack method \
             nativePtr sig J failed"
        );
        FOREIGN_CLASS_LOCALMEDIATRACK_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/DeviceVideoTrackConstraints"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/DeviceVideoTrackConstraints"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/DeviceVideoTrackConstraints"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class \
             com/jason/api/DeviceVideoTrackConstraints method nativePtr sig J \
             failed"
        );
        FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/Jason"));
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/Jason"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(!class.is_null(), "FindClass failed for com/jason/api/Jason");
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_JASON = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/Jason method nativePtr \
             sig J failed"
        );
        FOREIGN_CLASS_JASON_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
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
            "GetMethodID for class java/lang/Byte method byteValue sig ()B \
             failed"
        );
        JAVA_LANG_BYTE_BYTE_VALUE = method_id;
    }
    unsafe {
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
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/ConnectionHandle"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/ConnectionHandle"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/ConnectionHandle"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_CONNECTIONHANDLE = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/ConnectionHandle method \
             nativePtr sig J failed"
        );
        FOREIGN_CLASS_CONNECTIONHANDLE_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("java/util/OptionalLong"),
        );
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
            "GetStaticMethodID for class java/util/OptionalLong method empty \
             sig ()Ljava/util/OptionalLong; failed"
        );
        JAVA_UTIL_OPTIONAL_LONG_EMPTY = method_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/MediaKind"),
        );
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
            "GetStaticFieldID for class com/jason/api/MediaKind method Audio \
             sig Lcom/jason/api/MediaKind; failed"
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
            "GetStaticFieldID for class com/jason/api/MediaKind method Video \
             sig Lcom/jason/api/MediaKind; failed"
        );
        FOREIGN_ENUM_MEDIAKIND_VIDEO = field_id;
    }
    unsafe {
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
            "GetMethodID for class java/lang/Long method longValue sig ()J \
             failed"
        );
        JAVA_LANG_LONG_LONG_VALUE = method_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/DisplayVideoTrackConstraints"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/DisplayVideoTrackConstraints"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/DisplayVideoTrackConstraints"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class \
             com/jason/api/DisplayVideoTrackConstraints method nativePtr sig \
             J failed"
        );
        FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
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
    }
    unsafe {
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
    }
    unsafe {
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
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/RoomHandle"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/RoomHandle"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/RoomHandle"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_ROOMHANDLE = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/RoomHandle method \
             nativePtr sig J failed"
        );
        FOREIGN_CLASS_ROOMHANDLE_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/AudioTrackConstraints"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/AudioTrackConstraints"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/AudioTrackConstraints"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/AudioTrackConstraints \
             method nativePtr sig J failed"
        );
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/InputDeviceInfo"),
        );
        assert!(
            !class_local_ref.is_null(),
            "FindClass failed for com/jason/api/InputDeviceInfo"
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            "FindClass failed for com/jason/api/InputDeviceInfo"
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_INPUTDEVICEINFO = class;
        let field_id: jfieldID = (**env).GetFieldID.unwrap()(
            env,
            class,
            swig_c_str!("nativePtr"),
            swig_c_str!("J"),
        );
        assert!(
            !field_id.is_null(),
            "GetStaticFieldID for class com/jason/api/InputDeviceInfo method \
             nativePtr sig J failed"
        );
        FOREIGN_CLASS_INPUTDEVICEINFO_NATIVEPTR_FIELD = field_id;
    }
    unsafe {
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
    }
    JNI_VERSION
}

#[no_mangle]
pub extern "system" fn JNI_OnUnload(
    java_vm: *mut JavaVM,
    _reserved: *mut ::std::os::raw::c_void,
) {
    assert!(!java_vm.is_null());
    let mut env: *mut JNIEnv = ::std::ptr::null_mut();
    let res = unsafe {
        (**java_vm).GetEnv.unwrap()(
            java_vm,
            (&mut env) as *mut *mut JNIEnv as *mut *mut ::std::os::raw::c_void,
            JNI_VERSION,
        )
    };
    if res != (JNI_OK as jint) {
        panic!("JNI GetEnv in JNI_OnLoad failed, return code {}", res);
    }
    assert!(!env.is_null());
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_UTIL_OPTIONAL_INT);
        JAVA_UTIL_OPTIONAL_INT = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(
            env,
            FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION,
        );
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_REMOTEMEDIATRACK);
        FOREIGN_CLASS_REMOTEMEDIATRACK = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_JASONERROR);
        FOREIGN_CLASS_JASONERROR = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_UTIL_OPTIONAL_DOUBLE);
        JAVA_UTIL_OPTIONAL_DOUBLE = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_MEDIAMANAGERHANDLE);
        FOREIGN_CLASS_MEDIAMANAGERHANDLE = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_ROOMCLOSEREASON);
        FOREIGN_CLASS_ROOMCLOSEREASON = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_RECONNECTHANDLE);
        FOREIGN_CLASS_RECONNECTHANDLE = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_SHORT);
        JAVA_LANG_SHORT = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(
            env,
            FOREIGN_CLASS_MEDIASTREAMSETTINGS,
        );
        FOREIGN_CLASS_MEDIASTREAMSETTINGS = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_DOUBLE);
        JAVA_LANG_DOUBLE = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_ENUM_FACINGMODE);
        FOREIGN_ENUM_FACINGMODE = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_LOCALMEDIATRACK);
        FOREIGN_CLASS_LOCALMEDIATRACK = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(
            env,
            FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS,
        );
        FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_JASON);
        FOREIGN_CLASS_JASON = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_BYTE);
        JAVA_LANG_BYTE = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_INTEGER);
        JAVA_LANG_INTEGER = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_CONNECTIONHANDLE);
        FOREIGN_CLASS_CONNECTIONHANDLE = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_UTIL_OPTIONAL_LONG);
        JAVA_UTIL_OPTIONAL_LONG = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_ENUM_MEDIAKIND);
        FOREIGN_ENUM_MEDIAKIND = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_LONG);
        JAVA_LANG_LONG = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(
            env,
            FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS,
        );
        FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_EXCEPTION);
        JAVA_LANG_EXCEPTION = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_ENUM_MEDIASOURCEKIND);
        FOREIGN_ENUM_MEDIASOURCEKIND = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_FLOAT);
        JAVA_LANG_FLOAT = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_ROOMHANDLE);
        FOREIGN_CLASS_ROOMHANDLE = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(
            env,
            FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS,
        );
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_INPUTDEVICEINFO);
        FOREIGN_CLASS_INPUTDEVICEINFO = ::std::ptr::null_mut()
    }
    unsafe {
        (**env).DeleteGlobalRef.unwrap()(env, JAVA_LANG_STRING);
        JAVA_LANG_STRING = ::std::ptr::null_mut()
    }
}
