#![allow(clippy::not_unsafe_ptr_arg_deref, clippy::missing_safety_doc)]

use ndk_sys::*;

use crate::*;

#[repr(transparent)]
pub struct JForeignObjectsArray<T: SwigForeignClass> {
    inner: jobjectArray,
    _marker: ::std::marker::PhantomData<T>,
}

#[doc = " Default JNI_VERSION"]
const JNI_VERSION: jint = JNI_VERSION_1_6 as jint;
#[doc = " Marker for what to cache in JNI_OnLoad"]
macro_rules! swig_jni_find_class {
    ($ id : ident , $ path : expr) => {
        unsafe { $id }
    };
    ($ id : ident , $ path : expr ,) => {
        unsafe { $id }
    };
}
macro_rules! swig_jni_get_field_id {
    ($ global_id : ident , $ class_id : ident , $ name : expr , $ sig : expr) => {
        unsafe { $global_id }
    };
    ($ global_id : ident , $ class_id : ident , $ name : expr , $ sig : expr ,) => {
        unsafe { $global_id }
    };
}
macro_rules! swig_jni_get_static_field_id {
    ($ global_id : ident , $ class_id : ident , $ name : expr , $ sig : expr) => {
        unsafe { $global_id }
    };
    ($ global_id : ident , $ class_id : ident , $ name : expr , $ sig : expr ,) => {
        unsafe { $global_id }
    };
}
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
macro_rules ! swig_assert_eq_size { ($ x : ty , $ ($ xs : ty) ,+ $ (,) *) => { $ (let _ = :: std :: mem :: transmute ::<$ x , $ xs >;) + } ; }
#[cfg(target_pointer_width = "32")]
pub unsafe fn jlong_to_pointer<T>(val: jlong) -> *mut T {
    (val as u32) as *mut T
}
#[cfg(target_pointer_width = "64")]
pub unsafe fn jlong_to_pointer<T>(val: jlong) -> *mut T {
    val as *mut T
}
pub trait SwigForeignClass {
    type PointedType;
    fn jni_class() -> jclass;
    fn jni_class_pointer_field() -> jfieldID;
    fn box_object(x: Self) -> jlong;
    fn unbox_object(x: jlong) -> Self;
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType>;
}
pub trait SwigForeignCLikeEnum {
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
            unsafe { (**env).GetStringUTFChars.unwrap()(env, js, ::std::ptr::null_mut()) }
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
                (**self.env).ReleaseStringUTFChars.unwrap()(self.env, self.string, self.chars)
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
                (**self.callback.java_vm).DetachCurrentThread.unwrap()(self.callback.java_vm)
            };
            if res != 0 {
                log::error!("JniEnvHolder: DetachCurrentThread failed: {}", res);
            }
        }
    }
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
                (&mut env) as *mut *mut JNIEnv as *mut *mut ::std::os::raw::c_void,
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
fn jni_throw(env: *mut JNIEnv, ex_class: jclass, message: &str) {
    let c_message = ::std::ffi::CString::new(message).unwrap();
    let res = unsafe { (**env).ThrowNew.unwrap()(env, ex_class, c_message.as_ptr()) };
    if res != 0 {
        log::error!(
            "JNI ThrowNew({}) failed for class {:?} failed",
            message,
            ex_class
        );
    }
}
fn jni_throw_exception(env: *mut JNIEnv, message: &str) {
    let exception_class = swig_jni_find_class!(JAVA_LANG_EXCEPTION, "java/lang/Exception");
    jni_throw(env, exception_class, message)
}
fn object_to_jobject<T: SwigForeignClass>(env: *mut JNIEnv, obj: T) -> jobject {
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
            panic!("object_to_jobject: Can not set nativePtr field: catch exception");
        }
    }
    jobj
}
fn vec_of_objects_to_jobject_array<T: SwigForeignClass>(
    env: *mut JNIEnv,
    mut arr: Vec<T>,
) -> JForeignObjectsArray<T> {
    let jcls: jclass = <T>::jni_class();
    assert!(!jcls.is_null());
    let arr_len = <jsize as ::std::convert::TryFrom<usize>>::try_from(arr.len())
        .expect("invalid usize, in usize => to jsize conversation");
    let obj_arr: jobjectArray =
        unsafe { (**env).NewObjectArray.unwrap()(env, arr_len, jcls, ::std::ptr::null_mut()) };
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
            (**env).SetObjectArrayElement.unwrap()(env, obj_arr, i as jsize, jobj);
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
impl<T: SwigForeignClass> JniInvalidValue for JForeignObjectsArray<T> {
    fn jni_invalid_value() -> Self {
        Self {
            inner: ::std::ptr::null_mut(),
            _marker: ::std::marker::PhantomData,
        }
    }
}
macro_rules ! impl_jni_jni_invalid_value { ($ ($ type : ty) *) => ($ (impl JniInvalidValue for $ type { fn jni_invalid_value () -> Self { <$ type >:: default () } }) *) }
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
macro_rules ! define_array_handling_code { ($ ([jni_arr_type = $ jni_arr_type : ident , rust_arr_wrapper = $ rust_arr_wrapper : ident , jni_get_array_elements = $ jni_get_array_elements : ident , jni_elem_type = $ jni_elem_type : ident , rust_elem_type = $ rust_elem_type : ident , jni_release_array_elements = $ jni_release_array_elements : ident , jni_new_array = $ jni_new_array : ident , jni_set_array_region = $ jni_set_array_region : ident]) ,*) => { $ (# [allow (dead_code)] struct $ rust_arr_wrapper { array : $ jni_arr_type , data : * mut $ jni_elem_type , env : * mut JNIEnv , } # [allow (dead_code)] impl $ rust_arr_wrapper { fn new (env : * mut JNIEnv , array : $ jni_arr_type) -> $ rust_arr_wrapper { assert ! (! array . is_null ()) ; let data = unsafe { (** env) .$ jni_get_array_elements . unwrap () (env , array , :: std :: ptr :: null_mut ()) } ; $ rust_arr_wrapper { array , data , env } } fn to_slice (& self) -> & [$ rust_elem_type] { unsafe { let len : jsize = (** self . env) . GetArrayLength . unwrap () (self . env , self . array) ; assert ! ((len as u64) <= (usize :: max_value () as u64)) ; :: std :: slice :: from_raw_parts (self . data , len as usize) } } fn from_slice_to_raw (arr : & [$ rust_elem_type] , env : * mut JNIEnv) -> $ jni_arr_type { assert ! ((arr . len () as u64) <= (jsize :: max_value () as u64)) ; let jarr : $ jni_arr_type = unsafe { (** env) .$ jni_new_array . unwrap () (env , arr . len () as jsize) } ; assert ! (! jarr . is_null ()) ; unsafe { (** env) .$ jni_set_array_region . unwrap () (env , jarr , 0 , arr . len () as jsize , arr . as_ptr ()) ; if (** env) . ExceptionCheck . unwrap () (env) != 0 { panic ! ("{}:{} {} failed" , file ! () , line ! () , stringify ! ($ jni_set_array_region)) ; } } jarr } } # [allow (dead_code)] impl Drop for $ rust_arr_wrapper { fn drop (& mut self) { assert ! (! self . env . is_null ()) ; assert ! (! self . array . is_null ()) ; unsafe { (** self . env) .$ jni_release_array_elements . unwrap () (self . env , self . array , self . data , JNI_ABORT as jint ,) } ; } }) * } }
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

#[no_mangle]
pub extern "C" fn Java_com_jason_api_JNIReachabilityFence_reachabilityFence1(
    _: *mut JNIEnv,
    _: jclass,
    _: jobject,
) {
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JNIReachabilityFence_reachabilityFence2(
    _: *mut JNIEnv,
    _: jclass,
    _: jobject,
    _: jobject,
) {
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JNIReachabilityFence_reachabilityFence3(
    _: *mut JNIEnv,
    _: jclass,
    _: jobject,
    _: jobject,
    _: jobject,
) {
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JNIReachabilityFence_reachabilityFence4(
    _: *mut JNIEnv,
    _: jclass,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
) {
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JNIReachabilityFence_reachabilityFence5(
    _: *mut JNIEnv,
    _: jclass,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
) {
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JNIReachabilityFence_reachabilityFence6(
    _: *mut JNIEnv,
    _: jclass,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
) {
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JNIReachabilityFence_reachabilityFence7(
    _: *mut JNIEnv,
    _: jclass,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
) {
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JNIReachabilityFence_reachabilityFence8(
    _: *mut JNIEnv,
    _: jclass,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
    _: jobject,
) {
}
impl SwigForeignCLikeEnum for FacingMode {
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
            _ => panic!(concat!("{} not expected for ", stringify!(FacingMode)), x),
        }
    }
}
impl SwigFrom<FacingMode> for jobject {
    fn swig_from(x: FacingMode, env: *mut JNIEnv) -> jobject {
        let cls: jclass = swig_jni_find_class!(FOREIGN_ENUM_FACINGMODE, "com/jason/api/FacingMode");
        assert!(!cls.is_null());
        let static_field_id: jfieldID = match x {
            FacingMode::User => {
                let field = swig_jni_get_static_field_id!(
                    FOREIGN_ENUM_FACINGMODE_USER,
                    FOREIGN_ENUM_FACINGMODE,
                    "User",
                    "Lcom/jason/api/FacingMode;"
                );
                assert!(!field.is_null());
                field
            }
            FacingMode::Environment => {
                let field = swig_jni_get_static_field_id!(
                    FOREIGN_ENUM_FACINGMODE_IRONMENT,
                    FOREIGN_ENUM_FACINGMODE,
                    "Environment",
                    "Lcom/jason/api/FacingMode;"
                );
                assert!(!field.is_null());
                field
            }
            FacingMode::Left => {
                let field = swig_jni_get_static_field_id!(
                    FOREIGN_ENUM_FACINGMODE_LEFT,
                    FOREIGN_ENUM_FACINGMODE,
                    "Left",
                    "Lcom/jason/api/FacingMode;"
                );
                assert!(!field.is_null());
                field
            }
            FacingMode::Right => {
                let field = swig_jni_get_static_field_id!(
                    FOREIGN_ENUM_FACINGMODE_RIGHT,
                    FOREIGN_ENUM_FACINGMODE,
                    "Right",
                    "Lcom/jason/api/FacingMode;"
                );
                assert!(!field.is_null());
                field
            }
        };
        assert!(!static_field_id.is_null());
        let ret: jobject =
            unsafe { (**env).GetStaticObjectField.unwrap()(env, cls, static_field_id) };
        assert!(
            !ret.is_null(),
            concat!("Can get value of item in ", "com/jason/api/FacingMode")
        );
        ret
    }
}
impl SwigForeignCLikeEnum for MediaKind {
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
            _ => panic!(concat!("{} not expected for ", stringify!(MediaKind)), x),
        }
    }
}
impl SwigFrom<MediaKind> for jobject {
    fn swig_from(x: MediaKind, env: *mut JNIEnv) -> jobject {
        let cls: jclass = swig_jni_find_class!(FOREIGN_ENUM_MEDIAKIND, "com/jason/api/MediaKind");
        assert!(!cls.is_null());
        let static_field_id: jfieldID = match x {
            MediaKind::Audio => {
                let field = swig_jni_get_static_field_id!(
                    FOREIGN_ENUM_MEDIAKIND_AUDIO,
                    FOREIGN_ENUM_MEDIAKIND,
                    "Audio",
                    "Lcom/jason/api/MediaKind;"
                );
                assert!(!field.is_null());
                field
            }
            MediaKind::Video => {
                let field = swig_jni_get_static_field_id!(
                    FOREIGN_ENUM_MEDIAKIND_VIDEO,
                    FOREIGN_ENUM_MEDIAKIND,
                    "Video",
                    "Lcom/jason/api/MediaKind;"
                );
                assert!(!field.is_null());
                field
            }
        };
        assert!(!static_field_id.is_null());
        let ret: jobject =
            unsafe { (**env).GetStaticObjectField.unwrap()(env, cls, static_field_id) };
        assert!(
            !ret.is_null(),
            concat!("Can get value of item in ", "com/jason/api/MediaKind")
        );
        ret
    }
}
impl SwigForeignCLikeEnum for MediaSourceKind {
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
        let cls: jclass = swig_jni_find_class!(
            FOREIGN_ENUM_MEDIASOURCEKIND,
            "com/jason/api/MediaSourceKind"
        );
        assert!(!cls.is_null());
        let static_field_id: jfieldID = match x {
            MediaSourceKind::Device => {
                let field = swig_jni_get_static_field_id!(
                    FOREIGN_ENUM_MEDIASOURCEKIND_DEVICE,
                    FOREIGN_ENUM_MEDIASOURCEKIND,
                    "Device",
                    "Lcom/jason/api/MediaSourceKind;"
                );
                assert!(!field.is_null());
                field
            }
            MediaSourceKind::Display => {
                let field = swig_jni_get_static_field_id!(
                    FOREIGN_ENUM_MEDIASOURCEKIND_DISPLAY,
                    FOREIGN_ENUM_MEDIASOURCEKIND,
                    "Display",
                    "Lcom/jason/api/MediaSourceKind;"
                );
                assert!(!field.is_null());
                field
            }
        };
        assert!(!static_field_id.is_null());
        let ret: jobject =
            unsafe { (**env).GetStaticObjectField.unwrap()(env, cls, static_field_id) };
        assert!(
            !ret.is_null(),
            concat!("Can get value of item in ", "com/jason/api/MediaSourceKind")
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
                (**env).CallVoidMethod.unwrap()(env, self.this, self.methods[0usize]);
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(stringify!(call), ": java throw exception"));
                    (**env).ExceptionDescribe.unwrap()(env);
                    (**env).ExceptionClear.unwrap()(env);
                }
            };
        }
    }
}
impl SwigForeignClass for RemoteMediaTrack {
    type PointedType = RemoteMediaTrack;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_REMOTEMEDIATRACK,
            "com/jason/api/RemoteMediaTrack"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_REMOTEMEDIATRACK_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_REMOTEMEDIATRACK,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<RemoteMediaTrack> = Box::new(this);
        let this: *mut RemoteMediaTrack = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut RemoteMediaTrack =
            unsafe { jlong_to_pointer::<RemoteMediaTrack>(x).as_mut().unwrap() };
        let x: Box<RemoteMediaTrack> = unsafe { Box::from_raw(x) };
        let x: RemoteMediaTrack = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut RemoteMediaTrack =
            unsafe { jlong_to_pointer::<RemoteMediaTrack>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeEnabled(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: bool = RemoteMediaTrack::enabled(this);
    ret as jboolean
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
            (**env).GetMethodID.unwrap()(env, class, swig_c_str!("call"), swig_c_str!("()V"))
        };
        assert!(!method_id.is_null(), "Can not find call id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeOnEnabled(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    callback: jobject,
) {
    let callback: Box<dyn Callback> = <Box<dyn Callback>>::swig_from(callback, env);
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: () = RemoteMediaTrack::on_enabled(this, callback);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeOnDisabled(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    callback: jobject,
) {
    let callback: Box<dyn Callback> = <Box<dyn Callback>>::swig_from(callback, env);
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: () = RemoteMediaTrack::on_disabled(this, callback);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: MediaKind = RemoteMediaTrack::kind(this);
    let ret: jint = ret.as_jint();
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeMediaSourceKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let ret: MediaSourceKind = RemoteMediaTrack::media_source_kind(this);
    let ret: jint = ret.as_jint();
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RemoteMediaTrack_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut RemoteMediaTrack =
        unsafe { jlong_to_pointer::<RemoteMediaTrack>(this).as_mut().unwrap() };
    let this: Box<RemoteMediaTrack> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl Consumer<RemoteMediaTrack> for JavaCallback {
    fn accept(&self, a0: RemoteMediaTrack) {
        swig_assert_eq_size!(::std::os::raw::c_uint, u32);
        swig_assert_eq_size!(::std::os::raw::c_int, i32);
        let env = self.get_jni_();
        if let Some(env) = env.env {
            let a0: jobject = object_to_jobject(env, a0);
            unsafe {
                (**env).CallVoidMethod.unwrap()(env, self.this, self.methods[0usize], a0);
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(stringify!(accept), ": java throw exception"));
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
                    log::error!(concat!(stringify!(accept), ": java throw exception"));
                    (**env).ExceptionDescribe.unwrap()(env);
                    (**env).ExceptionClear.unwrap()(env);
                }
            };
        }
    }
}
impl SwigForeignClass for RoomCloseReason {
    type PointedType = RoomCloseReason;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_ROOMCLOSEREASON,
            "com/jason/api/RoomCloseReason"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_ROOMCLOSEREASON_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_ROOMCLOSEREASON,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<RoomCloseReason> = Box::new(this);
        let this: *mut RoomCloseReason = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut RoomCloseReason =
            unsafe { jlong_to_pointer::<RoomCloseReason>(x).as_mut().unwrap() };
        let x: Box<RoomCloseReason> = unsafe { Box::from_raw(x) };
        let x: RoomCloseReason = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut RoomCloseReason =
            unsafe { jlong_to_pointer::<RoomCloseReason>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeReason(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &RoomCloseReason =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    let ret: String = RoomCloseReason::reason(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeIsClosedByServer(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    let this: &RoomCloseReason =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    let ret: bool = RoomCloseReason::is_closed_by_server(this);
    ret as jboolean
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeRoomCloseReason(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jboolean {
    let this: &RoomCloseReason =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    let ret: bool = RoomCloseReason::is_err(this);
    ret as jboolean
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomCloseReason_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut RoomCloseReason =
        unsafe { jlong_to_pointer::<RoomCloseReason>(this).as_mut().unwrap() };
    let this: Box<RoomCloseReason> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for Jason {
    type PointedType = Jason;
    fn jni_class() -> jclass {
        swig_jni_find_class!(FOREIGN_CLASS_JASON, "com/jason/api/Jason")
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_JASON_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_JASON,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<Jason> = Box::new(this);
        let this: *mut Jason = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut Jason = unsafe { jlong_to_pointer::<Jason>(x).as_mut().unwrap() };
        let x: Box<Jason> = unsafe { Box::from_raw(x) };
        let x: Jason = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut Jason = unsafe { jlong_to_pointer::<Jason>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_init(_: *mut JNIEnv, _: jclass) -> jlong {
    let this: Jason = Jason::new();
    let this: Box<Jason> = Box::new(this);
    let this: *mut Jason = Box::into_raw(this);
    this as jlong
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeInitRoom(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let this: &Jason = unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
    let ret: RoomHandle = Jason::init_room(this);
    let ret: jlong = <RoomHandle>::box_object(ret);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeMediaManager(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jlong {
    let this: &Jason = unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
    let ret: MediaManagerHandle = Jason::media_manager(this);
    let ret: jlong = <MediaManagerHandle>::box_object(ret);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeCloseRoom(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    room_to_delete: jlong,
) {
    let room_to_delete: *mut RoomHandle = unsafe {
        jlong_to_pointer::<RoomHandle>(room_to_delete)
            .as_mut()
            .unwrap()
    };
    let room_to_delete: Box<RoomHandle> = unsafe { Box::from_raw(room_to_delete) };
    let room_to_delete: RoomHandle = *room_to_delete;
    let this: &Jason = unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
    let ret: () = Jason::close_room(this, room_to_delete);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_Jason_nativeFree(_: *mut JNIEnv, _: jclass, this: jlong) {
    let this: *mut Jason = unsafe { jlong_to_pointer::<Jason>(this).as_mut().unwrap() };
    let this: Box<Jason> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for ConnectionHandle {
    type PointedType = ConnectionHandle;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_CONNECTIONHANDLE,
            "com/jason/api/ConnectionHandle"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_CONNECTIONHANDLE_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_CONNECTIONHANDLE,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<ConnectionHandle> = Box::new(this);
        let this: *mut ConnectionHandle = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut ConnectionHandle =
            unsafe { jlong_to_pointer::<ConnectionHandle>(x).as_mut().unwrap() };
        let x: Box<ConnectionHandle> = unsafe { Box::from_raw(x) };
        let x: ConnectionHandle = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut ConnectionHandle =
            unsafe { jlong_to_pointer::<ConnectionHandle>(x).as_mut().unwrap() };
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
    let ret: Result<String, String> = ConnectionHandle::get_remote_member_id(this);
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
    let ret: Result<(), String> = ConnectionHandle::on_remote_track_added(this, f);
    let ret: () = match ret {
        Ok(x) => x,
        Err(msg) => {
            jni_throw_exception(env, &msg);
            return <()>::jni_invalid_value();
        }
    };
    ret
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
            (**env).GetMethodID.unwrap()(env, class, swig_c_str!("accept"), swig_c_str!("(S)V"))
        };
        assert!(!method_id.is_null(), "Can not find accept id");
        cb.methods.push(method_id);
        Box::new(cb)
    }
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
    let ret: Result<(), String> = ConnectionHandle::on_quality_score_update(this, f);
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
impl SwigForeignClass for ReconnectHandle {
    type PointedType = ReconnectHandle;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_RECONNECTHANDLE,
            "com/jason/api/ReconnectHandle"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_RECONNECTHANDLE_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_RECONNECTHANDLE,
            "nativePtr",
            "J"
        )
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
    let delay_ms: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(delay_ms)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &ReconnectHandle =
        unsafe { jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> = ReconnectHandle::reconnect_with_delay(this, delay_ms);
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
    let max_delay: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(max_delay)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &ReconnectHandle =
        unsafe { jlong_to_pointer::<ReconnectHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> =
        ReconnectHandle::reconnect_with_backoff(this, starting_delay_ms, multiplier, max_delay);
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
impl SwigForeignClass for JasonError {
    type PointedType = JasonError;
    fn jni_class() -> jclass {
        swig_jni_find_class!(FOREIGN_CLASS_JASONERROR, "com/jason/api/JasonError")
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_JASONERROR_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_JASONERROR,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<JasonError> = Box::new(this);
        let this: *mut JasonError = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut JasonError = unsafe { jlong_to_pointer::<JasonError>(x).as_mut().unwrap() };
        let x: Box<JasonError> = unsafe { Box::from_raw(x) };
        let x: JasonError = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut JasonError = unsafe { jlong_to_pointer::<JasonError>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeName(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &JasonError = unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
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
    let this: &JasonError = unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
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
    let this: &JasonError = unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
    let ret: String = JasonError::trace(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_JasonError_nativeFree(_: *mut JNIEnv, _: jclass, this: jlong) {
    let this: *mut JasonError = unsafe { jlong_to_pointer::<JasonError>(this).as_mut().unwrap() };
    let this: Box<JasonError> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for LocalMediaTrack {
    type PointedType = LocalMediaTrack;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_LOCALMEDIATRACK,
            "com/jason/api/LocalMediaTrack"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_LOCALMEDIATRACK_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_LOCALMEDIATRACK,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<LocalMediaTrack> = Box::new(this);
        let this: *mut LocalMediaTrack = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut LocalMediaTrack =
            unsafe { jlong_to_pointer::<LocalMediaTrack>(x).as_mut().unwrap() };
        let x: Box<LocalMediaTrack> = unsafe { Box::from_raw(x) };
        let x: LocalMediaTrack = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut LocalMediaTrack =
            unsafe { jlong_to_pointer::<LocalMediaTrack>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &LocalMediaTrack =
        unsafe { jlong_to_pointer::<LocalMediaTrack>(this).as_mut().unwrap() };
    let ret: MediaKind = LocalMediaTrack::kind(this);
    let ret: jint = ret.as_jint();
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeMediaSourceKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &LocalMediaTrack =
        unsafe { jlong_to_pointer::<LocalMediaTrack>(this).as_mut().unwrap() };
    let ret: MediaSourceKind = LocalMediaTrack::media_source_kind(this);
    let ret: jint = ret.as_jint();
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_LocalMediaTrack_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut LocalMediaTrack =
        unsafe { jlong_to_pointer::<LocalMediaTrack>(this).as_mut().unwrap() };
    let this: Box<LocalMediaTrack> = unsafe { Box::from_raw(this) };
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
                (**env).CallVoidMethod.unwrap()(env, self.this, self.methods[0usize], a0);
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(stringify!(accept), ": java throw exception"));
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
                (**env).CallVoidMethod.unwrap()(env, self.this, self.methods[0usize], a0);
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(stringify!(accept), ": java throw exception"));
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
                (**env).CallVoidMethod.unwrap()(env, self.this, self.methods[0usize], a0);
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(stringify!(accept), ": java throw exception"));
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
                (**env).CallVoidMethod.unwrap()(env, self.this, self.methods[0usize], a0);
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(stringify!(accept), ": java throw exception"));
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
                (**env).CallVoidMethod.unwrap()(env, self.this, self.methods[0usize], a0);
                if (**env).ExceptionCheck.unwrap()(env) != 0 {
                    log::error!(concat!(stringify!(accept), ": java throw exception"));
                    (**env).ExceptionDescribe.unwrap()(env);
                    (**env).ExceptionClear.unwrap()(env);
                }
            };
        }
    }
}
impl SwigForeignClass for RoomHandle {
    type PointedType = RoomHandle;
    fn jni_class() -> jclass {
        swig_jni_find_class!(FOREIGN_CLASS_ROOMHANDLE, "com/jason/api/RoomHandle")
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_ROOMHANDLE_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_ROOMHANDLE,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<RoomHandle> = Box::new(this);
        let this: *mut RoomHandle = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(x).as_mut().unwrap() };
        let x: Box<RoomHandle> = unsafe { Box::from_raw(x) };
        let x: RoomHandle = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(x).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnNewConnection(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<ConnectionHandle>> =
        <Box<dyn Consumer<ConnectionHandle>>>::swig_from(a0, env);
    let this: &mut RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnClose(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<RoomCloseReason>> =
        <Box<dyn Consumer<RoomCloseReason>>>::swig_from(a0, env);
    let this: &mut RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnLocalTrack(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<LocalMediaTrack>> =
        <Box<dyn Consumer<LocalMediaTrack>>>::swig_from(a0, env);
    let this: &mut RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnFailedLocalMedia(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<JasonError>> = <Box<dyn Consumer<JasonError>>>::swig_from(a0, env);
    let this: &mut RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
#[no_mangle]
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeOnConnectionLoss(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    a0: jobject,
) {
    let a0: Box<dyn Consumer<ReconnectHandle>> =
        <Box<dyn Consumer<ReconnectHandle>>>::swig_from(a0, env);
    let this: &mut RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let ret: Result<(), String> =
        RoomHandle::set_local_media_settings(this, settings, stop_first, rollback_on_fail);
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
    let this: &RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
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
pub extern "C" fn Java_com_jason_api_RoomHandle_nativeFree(_: *mut JNIEnv, _: jclass, this: jlong) {
    let this: *mut RoomHandle = unsafe { jlong_to_pointer::<RoomHandle>(this).as_mut().unwrap() };
    let this: Box<RoomHandle> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for MediaManagerHandle {
    type PointedType = MediaManagerHandle;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_MEDIAMANAGERHANDLE,
            "com/jason/api/MediaManagerHandle"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_MEDIAMANAGERHANDLE_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_MEDIAMANAGERHANDLE,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<MediaManagerHandle> = Box::new(this);
        let this: *mut MediaManagerHandle = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut MediaManagerHandle =
            unsafe { jlong_to_pointer::<MediaManagerHandle>(x).as_mut().unwrap() };
        let x: Box<MediaManagerHandle> = unsafe { Box::from_raw(x) };
        let x: MediaManagerHandle = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut MediaManagerHandle =
            unsafe { jlong_to_pointer::<MediaManagerHandle>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeEnumerateDevices(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> JForeignObjectsArray<InputDeviceInfo> {
    let this: &MediaManagerHandle = unsafe {
        jlong_to_pointer::<MediaManagerHandle>(this)
            .as_mut()
            .unwrap()
    };
    let ret: Result<Vec<InputDeviceInfo>, String> = MediaManagerHandle::enumerate_devices(this);
    let ret: JForeignObjectsArray<InputDeviceInfo> = match ret {
        Ok(x) => {
            let ret: JForeignObjectsArray<InputDeviceInfo> =
                vec_of_objects_to_jobject_array(env, x);
            ret
        }
        Err(msg) => {
            jni_throw_exception(env, &msg);
            return <JForeignObjectsArray<InputDeviceInfo>>::jni_invalid_value();
        }
    };
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeInitLocalTracks(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    caps: jlong,
) -> JForeignObjectsArray<LocalMediaTrack> {
    let caps: &MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(caps)
            .as_mut()
            .unwrap()
    };
    let this: &MediaManagerHandle = unsafe {
        jlong_to_pointer::<MediaManagerHandle>(this)
            .as_mut()
            .unwrap()
    };
    let ret: Result<Vec<LocalMediaTrack>, String> =
        MediaManagerHandle::init_local_tracks(this, caps);
    let ret: JForeignObjectsArray<LocalMediaTrack> = match ret {
        Ok(x) => {
            let ret: JForeignObjectsArray<LocalMediaTrack> =
                vec_of_objects_to_jobject_array(env, x);
            ret
        }
        Err(msg) => {
            jni_throw_exception(env, &msg);
            return <JForeignObjectsArray<LocalMediaTrack>>::jni_invalid_value();
        }
    };
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaManagerHandle_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut MediaManagerHandle = unsafe {
        jlong_to_pointer::<MediaManagerHandle>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<MediaManagerHandle> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for InputDeviceInfo {
    type PointedType = InputDeviceInfo;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_INPUTDEVICEINFO,
            "com/jason/api/InputDeviceInfo"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_INPUTDEVICEINFO_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_INPUTDEVICEINFO,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<InputDeviceInfo> = Box::new(this);
        let this: *mut InputDeviceInfo = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut InputDeviceInfo =
            unsafe { jlong_to_pointer::<InputDeviceInfo>(x).as_mut().unwrap() };
        let x: Box<InputDeviceInfo> = unsafe { Box::from_raw(x) };
        let x: InputDeviceInfo = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut InputDeviceInfo =
            unsafe { jlong_to_pointer::<InputDeviceInfo>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeDeviceId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let ret: String = InputDeviceInfo::device_id(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeKind(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jint {
    let this: &InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let ret: MediaKind = InputDeviceInfo::kind(this);
    let ret: jint = ret.as_jint();
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeLabel(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let ret: String = InputDeviceInfo::label(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeGroupId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
) -> jstring {
    let this: &InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let ret: String = InputDeviceInfo::group_id(this);
    let ret: jstring = from_std_string_jstring(ret, env);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_InputDeviceInfo_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut InputDeviceInfo =
        unsafe { jlong_to_pointer::<InputDeviceInfo>(this).as_mut().unwrap() };
    let this: Box<InputDeviceInfo> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for MediaStreamSettings {
    type PointedType = MediaStreamSettings;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_MEDIASTREAMSETTINGS,
            "com/jason/api/MediaStreamSettings"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_MEDIASTREAMSETTINGS_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_MEDIASTREAMSETTINGS,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<MediaStreamSettings> = Box::new(this);
        let this: *mut MediaStreamSettings = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut MediaStreamSettings =
            unsafe { jlong_to_pointer::<MediaStreamSettings>(x).as_mut().unwrap() };
        let x: Box<MediaStreamSettings> = unsafe { Box::from_raw(x) };
        let x: MediaStreamSettings = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut MediaStreamSettings =
            unsafe { jlong_to_pointer::<MediaStreamSettings>(x).as_mut().unwrap() };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeAudio(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    let constraints: *mut AudioTrackConstraints = unsafe {
        jlong_to_pointer::<AudioTrackConstraints>(constraints)
            .as_mut()
            .unwrap()
    };
    let constraints: Box<AudioTrackConstraints> = unsafe { Box::from_raw(constraints) };
    let constraints: AudioTrackConstraints = *constraints;
    let this: &mut MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = MediaStreamSettings::audio(this, constraints);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeDeviceVideo(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    let constraints: *mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(constraints)
            .as_mut()
            .unwrap()
    };
    let constraints: Box<DeviceVideoTrackConstraints> = unsafe { Box::from_raw(constraints) };
    let constraints: DeviceVideoTrackConstraints = *constraints;
    let this: &mut MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = MediaStreamSettings::device_video(this, constraints);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeDisplayVideo(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    constraints: jlong,
) {
    let constraints: *mut DisplayVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DisplayVideoTrackConstraints>(constraints)
            .as_mut()
            .unwrap()
    };
    let constraints: Box<DisplayVideoTrackConstraints> = unsafe { Box::from_raw(constraints) };
    let constraints: DisplayVideoTrackConstraints = *constraints;
    let this: &mut MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = MediaStreamSettings::display_video(this, constraints);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_MediaStreamSettings_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut MediaStreamSettings = unsafe {
        jlong_to_pointer::<MediaStreamSettings>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<MediaStreamSettings> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for DisplayVideoTrackConstraints {
    type PointedType = DisplayVideoTrackConstraints;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS,
            "com/jason/api/DisplayVideoTrackConstraints"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<DisplayVideoTrackConstraints> = Box::new(this);
        let this: *mut DisplayVideoTrackConstraints = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut DisplayVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DisplayVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        let x: Box<DisplayVideoTrackConstraints> = unsafe { Box::from_raw(x) };
        let x: DisplayVideoTrackConstraints = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut DisplayVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DisplayVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DisplayVideoTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut DisplayVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DisplayVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<DisplayVideoTrackConstraints> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for AudioTrackConstraints {
    type PointedType = AudioTrackConstraints;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS,
            "com/jason/api/AudioTrackConstraints"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<AudioTrackConstraints> = Box::new(this);
        let this: *mut AudioTrackConstraints = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut AudioTrackConstraints = unsafe {
            jlong_to_pointer::<AudioTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        let x: Box<AudioTrackConstraints> = unsafe { Box::from_raw(x) };
        let x: AudioTrackConstraints = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut AudioTrackConstraints = unsafe {
            jlong_to_pointer::<AudioTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeDeviceId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    device_id: jstring,
) {
    let device_id: JavaString = JavaString::new(env, device_id);
    let device_id: &str = device_id.to_str();
    let device_id: String = device_id.to_string();
    let this: &mut AudioTrackConstraints = unsafe {
        jlong_to_pointer::<AudioTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = AudioTrackConstraints::device_id(this, device_id);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_AudioTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut AudioTrackConstraints = unsafe {
        jlong_to_pointer::<AudioTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<AudioTrackConstraints> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for DeviceVideoTrackConstraints {
    type PointedType = DeviceVideoTrackConstraints;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS,
            "com/jason/api/DeviceVideoTrackConstraints"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS,
            "nativePtr",
            "J"
        )
    }
    fn box_object(this: Self) -> jlong {
        let this: Box<DeviceVideoTrackConstraints> = Box::new(this);
        let this: *mut DeviceVideoTrackConstraints = Box::into_raw(this);
        this as jlong
    }
    fn unbox_object(x: jlong) -> Self {
        let x: *mut DeviceVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        let x: Box<DeviceVideoTrackConstraints> = unsafe { Box::from_raw(x) };
        let x: DeviceVideoTrackConstraints = *x;
        x
    }
    fn as_pointer(x: jlong) -> ::std::ptr::NonNull<Self::PointedType> {
        let x: *mut DeviceVideoTrackConstraints = unsafe {
            jlong_to_pointer::<DeviceVideoTrackConstraints>(x)
                .as_mut()
                .unwrap()
        };
        ::std::ptr::NonNull::<Self::PointedType>::new(x).unwrap()
    }
}

#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeDeviceId(
    env: *mut JNIEnv,
    _: jclass,
    this: jlong,
    device_id: jstring,
) {
    let device_id: JavaString = JavaString::new(env, device_id);
    let device_id: &str = device_id.to_str();
    let device_id: String = device_id.to_string();
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::device_id(this, device_id);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactFacingMode(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    facing_mode: jint,
) {
    let facing_mode: FacingMode = <FacingMode>::from_jint(facing_mode);
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::exact_facing_mode(this, facing_mode);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealFacingMode(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    facing_mode: jint,
) {
    let facing_mode: FacingMode = <FacingMode>::from_jint(facing_mode);
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::ideal_facing_mode(this, facing_mode);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactHeight(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    height: jlong,
) {
    let height: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(height)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::exact_height(this, height);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealHeight(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    height: jlong,
) {
    let height: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(height)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::ideal_height(this, height);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeHeightInRange(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    min: jlong,
    max: jlong,
) {
    let min: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(min)
        .expect("invalid jlong, in jlong => u32 conversation");
    let max: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(max)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::height_in_range(this, min, max);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeExactWidth(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    width: jlong,
) {
    let width: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(width)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::exact_width(this, width);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeIdealWidth(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    width: jlong,
) {
    let width: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(width)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::ideal_width(this, width);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeWidthInRange(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
    min: jlong,
    max: jlong,
) {
    let min: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(min)
        .expect("invalid jlong, in jlong => u32 conversation");
    let max: u32 = <u32 as ::std::convert::TryFrom<jlong>>::try_from(max)
        .expect("invalid jlong, in jlong => u32 conversation");
    let this: &mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let ret: () = DeviceVideoTrackConstraints::width_in_range(this, min, max);
    ret
}
#[no_mangle]
pub extern "C" fn Java_com_jason_api_DeviceVideoTrackConstraints_nativeFree(
    _: *mut JNIEnv,
    _: jclass,
    this: jlong,
) {
    let this: *mut DeviceVideoTrackConstraints = unsafe {
        jlong_to_pointer::<DeviceVideoTrackConstraints>(this)
            .as_mut()
            .unwrap()
    };
    let this: Box<DeviceVideoTrackConstraints> = unsafe { Box::from_raw(this) };
    drop(this);
}
impl SwigForeignClass for ConstraintsUpdateException {
    type PointedType = ConstraintsUpdateException;
    fn jni_class() -> jclass {
        swig_jni_find_class!(
            FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION,
            "com/jason/api/ConstraintsUpdateException"
        )
    }
    fn jni_class_pointer_field() -> jfieldID {
        swig_jni_get_field_id!(
            FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_MNATIVEOBJ_FIELD,
            FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION,
            "nativePtr",
            "J"
        )
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
    let ret: Option<JasonError> = ConstraintsUpdateException::recover_reason(this);
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
    let ret: Option<JasonError> = ConstraintsUpdateException::recover_fail_reasons(this);
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
static mut FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_MNATIVEOBJ_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_REMOTEMEDIATRACK: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_REMOTEMEDIATRACK_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_JASONERROR: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_JASONERROR_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE: jclass = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE_OF: jmethodID = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_DOUBLE_EMPTY: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_MEDIAMANAGERHANDLE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_MEDIAMANAGERHANDLE_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_ROOMCLOSEREASON: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_ROOMCLOSEREASON_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_RECONNECTHANDLE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_RECONNECTHANDLE_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut JAVA_LANG_SHORT: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_SHORT_SHORT_VALUE: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_MEDIASTREAMSETTINGS: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_MEDIASTREAMSETTINGS_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut JAVA_LANG_DOUBLE: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_DOUBLE_DOUBLE_VALUE_METHOD: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_USER: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_IRONMENT: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_LEFT: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_FACINGMODE_RIGHT: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_LOCALMEDIATRACK: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_LOCALMEDIATRACK_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut FOREIGN_CLASS_JASON: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_JASON_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut JAVA_LANG_BYTE: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_BYTE_BYTE_VALUE: jmethodID = ::std::ptr::null_mut();
static mut JAVA_LANG_INTEGER: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_INTEGER_INT_VALUE: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_CONNECTIONHANDLE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_CONNECTIONHANDLE_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG: jclass = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG_OF: jmethodID = ::std::ptr::null_mut();
static mut JAVA_UTIL_OPTIONAL_LONG_EMPTY: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND: jclass = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND_AUDIO: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIAKIND_VIDEO: jfieldID = ::std::ptr::null_mut();
static mut JAVA_LANG_LONG: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_LONG_LONG_VALUE: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD: jfieldID =
    ::std::ptr::null_mut();
static mut JAVA_LANG_EXCEPTION: jclass = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND: jclass = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND_DEVICE: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_ENUM_MEDIASOURCEKIND_DISPLAY: jfieldID = ::std::ptr::null_mut();
static mut JAVA_LANG_FLOAT: jclass = ::std::ptr::null_mut();
static mut JAVA_LANG_FLOAT_FLOAT_VALUE: jmethodID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_ROOMHANDLE: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_ROOMHANDLE_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_INPUTDEVICEINFO: jclass = ::std::ptr::null_mut();
static mut FOREIGN_CLASS_INPUTDEVICEINFO_MNATIVEOBJ_FIELD: jfieldID = ::std::ptr::null_mut();
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
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/util/OptionalInt"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/util/OptionalInt")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/util/OptionalInt")
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
            concat!(
                "GetStaticMethodID for class ",
                "java/util/OptionalInt",
                " method ",
                "of",
                " sig ",
                "(I)Ljava/util/OptionalInt;",
                " failed"
            )
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
            concat!(
                "GetStaticMethodID for class ",
                "java/util/OptionalInt",
                " method ",
                "empty",
                " sig ",
                "()Ljava/util/OptionalInt;",
                " failed"
            )
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
            concat!(
                "FindClass failed for ",
                "com/jason/api/ConstraintsUpdateException"
            )
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!(
                "FindClass failed for ",
                "com/jason/api/ConstraintsUpdateException"
            )
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/ConstraintsUpdateException",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/RemoteMediaTrack"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/RemoteMediaTrack")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/RemoteMediaTrack")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_REMOTEMEDIATRACK = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/RemoteMediaTrack",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_REMOTEMEDIATRACK_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/JasonError"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/JasonError")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/JasonError")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_JASONERROR = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/JasonError",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_JASONERROR_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("java/util/OptionalDouble"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/util/OptionalDouble")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/util/OptionalDouble")
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
            concat!(
                "GetStaticMethodID for class ",
                "java/util/OptionalDouble",
                " method ",
                "of",
                " sig ",
                "(D)Ljava/util/OptionalDouble;",
                " failed"
            )
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
            concat!(
                "GetStaticMethodID for class ",
                "java/util/OptionalDouble",
                " method ",
                "empty",
                " sig ",
                "()Ljava/util/OptionalDouble;",
                " failed"
            )
        );
        JAVA_UTIL_OPTIONAL_DOUBLE_EMPTY = method_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/MediaManagerHandle"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/MediaManagerHandle")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/MediaManagerHandle")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_MEDIAMANAGERHANDLE = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/MediaManagerHandle",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_MEDIAMANAGERHANDLE_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/RoomCloseReason"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/RoomCloseReason")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/RoomCloseReason")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_ROOMCLOSEREASON = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/RoomCloseReason",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_ROOMCLOSEREASON_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/ReconnectHandle"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/ReconnectHandle")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/ReconnectHandle")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_RECONNECTHANDLE = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/ReconnectHandle",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_RECONNECTHANDLE_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Short"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/lang/Short")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/lang/Short")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        JAVA_LANG_SHORT = class;
        let method_id: jmethodID =
            (**env).GetMethodID.unwrap()(env, class, swig_c_str!("shortValue"), swig_c_str!("()S"));
        assert!(
            !method_id.is_null(),
            concat!(
                "GetMethodID for class ",
                "java/lang/Short",
                " method ",
                "shortValue",
                " sig ",
                "()S",
                " failed"
            )
        );
        JAVA_LANG_SHORT_SHORT_VALUE = method_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/MediaStreamSettings"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/MediaStreamSettings")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/MediaStreamSettings")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_MEDIASTREAMSETTINGS = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/MediaStreamSettings",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_MEDIASTREAMSETTINGS_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Double"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/lang/Double")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/lang/Double")
        );
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
            concat!(
                "GetMethodID for class ",
                "java/lang/Double",
                " method ",
                "doubleValue",
                " sig ",
                "()D",
                " failed"
            )
        );
        JAVA_LANG_DOUBLE_DOUBLE_VALUE_METHOD = method_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/FacingMode"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/FacingMode")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/FacingMode")
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
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/FacingMode",
                " method ",
                "User",
                " sig ",
                "Lcom/jason/api/FacingMode;",
                " failed"
            )
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
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/FacingMode",
                " method ",
                "Environment",
                " sig ",
                "Lcom/jason/api/FacingMode;",
                " failed"
            )
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
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/FacingMode",
                " method ",
                "Left",
                " sig ",
                "Lcom/jason/api/FacingMode;",
                " failed"
            )
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
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/FacingMode",
                " method ",
                "Right",
                " sig ",
                "Lcom/jason/api/FacingMode;",
                " failed"
            )
        );
        FOREIGN_ENUM_FACINGMODE_RIGHT = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/LocalMediaTrack"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/LocalMediaTrack")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/LocalMediaTrack")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_LOCALMEDIATRACK = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/LocalMediaTrack",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_LOCALMEDIATRACK_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(
            env,
            swig_c_str!("com/jason/api/DeviceVideoTrackConstraints"),
        );
        assert!(
            !class_local_ref.is_null(),
            concat!(
                "FindClass failed for ",
                "com/jason/api/DeviceVideoTrackConstraints"
            )
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!(
                "FindClass failed for ",
                "com/jason/api/DeviceVideoTrackConstraints"
            )
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/DeviceVideoTrackConstraints",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/Jason"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/Jason")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/Jason")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_JASON = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/Jason",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_JASON_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Byte"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/lang/Byte")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/lang/Byte")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        JAVA_LANG_BYTE = class;
        let method_id: jmethodID =
            (**env).GetMethodID.unwrap()(env, class, swig_c_str!("byteValue"), swig_c_str!("()B"));
        assert!(
            !method_id.is_null(),
            concat!(
                "GetMethodID for class ",
                "java/lang/Byte",
                " method ",
                "byteValue",
                " sig ",
                "()B",
                " failed"
            )
        );
        JAVA_LANG_BYTE_BYTE_VALUE = method_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Integer"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/lang/Integer")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/lang/Integer")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        JAVA_LANG_INTEGER = class;
        let method_id: jmethodID =
            (**env).GetMethodID.unwrap()(env, class, swig_c_str!("intValue"), swig_c_str!("()I"));
        assert!(
            !method_id.is_null(),
            concat!(
                "GetMethodID for class ",
                "java/lang/Integer",
                " method ",
                "intValue",
                " sig ",
                "()I",
                " failed"
            )
        );
        JAVA_LANG_INTEGER_INT_VALUE = method_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/ConnectionHandle"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/ConnectionHandle")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/ConnectionHandle")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_CONNECTIONHANDLE = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/ConnectionHandle",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_CONNECTIONHANDLE_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("java/util/OptionalLong"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/util/OptionalLong")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/util/OptionalLong")
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
            concat!(
                "GetStaticMethodID for class ",
                "java/util/OptionalLong",
                " method ",
                "of",
                " sig ",
                "(J)Ljava/util/OptionalLong;",
                " failed"
            )
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
            concat!(
                "GetStaticMethodID for class ",
                "java/util/OptionalLong",
                " method ",
                "empty",
                " sig ",
                "()Ljava/util/OptionalLong;",
                " failed"
            )
        );
        JAVA_UTIL_OPTIONAL_LONG_EMPTY = method_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/MediaKind"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/MediaKind")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/MediaKind")
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
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/MediaKind",
                " method ",
                "Audio",
                " sig ",
                "Lcom/jason/api/MediaKind;",
                " failed"
            )
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
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/MediaKind",
                " method ",
                "Video",
                " sig ",
                "Lcom/jason/api/MediaKind;",
                " failed"
            )
        );
        FOREIGN_ENUM_MEDIAKIND_VIDEO = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Long"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/lang/Long")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/lang/Long")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        JAVA_LANG_LONG = class;
        let method_id: jmethodID =
            (**env).GetMethodID.unwrap()(env, class, swig_c_str!("longValue"), swig_c_str!("()J"));
        assert!(
            !method_id.is_null(),
            concat!(
                "GetMethodID for class ",
                "java/lang/Long",
                " method ",
                "longValue",
                " sig ",
                "()J",
                " failed"
            )
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
            concat!(
                "FindClass failed for ",
                "com/jason/api/DisplayVideoTrackConstraints"
            )
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!(
                "FindClass failed for ",
                "com/jason/api/DisplayVideoTrackConstraints"
            )
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/DisplayVideoTrackConstraints",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Exception"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/lang/Exception")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/lang/Exception")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        JAVA_LANG_EXCEPTION = class;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/MediaSourceKind"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/MediaSourceKind")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/MediaSourceKind")
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
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/MediaSourceKind",
                " method ",
                "Device",
                " sig ",
                "Lcom/jason/api/MediaSourceKind;",
                " failed"
            )
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
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/MediaSourceKind",
                " method ",
                "Display",
                " sig ",
                "Lcom/jason/api/MediaSourceKind;",
                " failed"
            )
        );
        FOREIGN_ENUM_MEDIASOURCEKIND_DISPLAY = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/Float"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/lang/Float")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/lang/Float")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        JAVA_LANG_FLOAT = class;
        let method_id: jmethodID =
            (**env).GetMethodID.unwrap()(env, class, swig_c_str!("floatValue"), swig_c_str!("()F"));
        assert!(
            !method_id.is_null(),
            concat!(
                "GetMethodID for class ",
                "java/lang/Float",
                " method ",
                "floatValue",
                " sig ",
                "()F",
                " failed"
            )
        );
        JAVA_LANG_FLOAT_FLOAT_VALUE = method_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/RoomHandle"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/RoomHandle")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/RoomHandle")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_ROOMHANDLE = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/RoomHandle",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_ROOMHANDLE_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/AudioTrackConstraints"));
        assert!(
            !class_local_ref.is_null(),
            concat!(
                "FindClass failed for ",
                "com/jason/api/AudioTrackConstraints"
            )
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!(
                "FindClass failed for ",
                "com/jason/api/AudioTrackConstraints"
            )
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/AudioTrackConstraints",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref =
            (**env).FindClass.unwrap()(env, swig_c_str!("com/jason/api/InputDeviceInfo"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "com/jason/api/InputDeviceInfo")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "com/jason/api/InputDeviceInfo")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        FOREIGN_CLASS_INPUTDEVICEINFO = class;
        let field_id: jfieldID =
            (**env).GetFieldID.unwrap()(env, class, swig_c_str!("nativePtr"), swig_c_str!("J"));
        assert!(
            !field_id.is_null(),
            concat!(
                "GetStaticFieldID for class ",
                "com/jason/api/InputDeviceInfo",
                " method ",
                "nativePtr",
                " sig ",
                "J",
                " failed"
            )
        );
        FOREIGN_CLASS_INPUTDEVICEINFO_MNATIVEOBJ_FIELD = field_id;
    }
    unsafe {
        let class_local_ref = (**env).FindClass.unwrap()(env, swig_c_str!("java/lang/String"));
        assert!(
            !class_local_ref.is_null(),
            concat!("FindClass failed for ", "java/lang/String")
        );
        let class = (**env).NewGlobalRef.unwrap()(env, class_local_ref);
        assert!(
            !class.is_null(),
            concat!("FindClass failed for ", "java/lang/String")
        );
        (**env).DeleteLocalRef.unwrap()(env, class_local_ref);
        JAVA_LANG_STRING = class;
    }
    JNI_VERSION
}
#[no_mangle]
pub extern "system" fn JNI_OnUnload(java_vm: *mut JavaVM, _reserved: *mut ::std::os::raw::c_void) {
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
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_CONSTRAINTSUPDATEEXCEPTION);
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
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_MEDIASTREAMSETTINGS);
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
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_DEVICEVIDEOTRACKCONSTRAINTS);
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
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_DISPLAYVIDEOTRACKCONSTRAINTS);
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
        (**env).DeleteGlobalRef.unwrap()(env, FOREIGN_CLASS_AUDIOTRACKCONSTRAINTS);
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
