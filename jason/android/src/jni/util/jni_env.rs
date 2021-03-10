use std::{convert::TryFrom as _, ffi::CStr, marker::PhantomData, ptr};

use jni::{
    objects::{GlobalRef, JClass, JMethodID, JObject, JString, JValue},
    signature::{JavaType, Primitive},
};
use jni_sys::{jmethodID, jobject, jobjectArray, jsize};

use crate::jni::{ForeignClass};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JNIEnv<'a> {
    ptr: jni::JNIEnv<'a>,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> JNIEnv<'a> {
    pub unsafe fn from_raw(ptr: *mut jni_sys::JNIEnv) -> Self {
        JNIEnv {
            ptr: jni::JNIEnv::from_raw(ptr).unwrap(),
            _lifetime: PhantomData,
        }
    }

    pub fn new_object_array<T: ForeignClass>(
        &self,
        arr: Vec<T>,
    ) -> JForeignObjectsArray<T> {
        let arr_len = jsize::try_from(arr.len())
            .expect("invalid usize, in usize => to jsize conversation");
        let jarr = self.ptr.new_long_array(arr_len).unwrap();
        let values: Vec<_> =
            arr.into_iter().map(|obj| obj.box_object()).collect();
        self.ptr.set_long_array_region(jarr, 0, &values).unwrap();

        JForeignObjectsArray {
            _inner: jarr,
            _marker: PhantomData,
        }
    }

    pub fn new_global_ref(&self, obj: JObject) -> GlobalRef {
        self.ptr.new_global_ref(obj).unwrap()
    }

    pub fn throw_new(&self, message: &str) {
        self.ptr.throw_new("java/lang/Exception", message)
            .unwrap();
    }

    pub fn get_method_id(
        &self,
        class: JClass,
        name: &str,
        sig: &str,
    ) -> jni::objects::JMethodID {
        self.ptr.get_method_id(class, name, sig).unwrap()
    }

    pub fn call_object_method(
        &self,
        object: jobject,
        method: jmethodID,
    ) -> JValue {
        self.ptr
            .call_method_unchecked(
                JObject::from(object),
                JMethodID::from(method),
                jni::signature::JavaType::Object(String::new()),
                &[],
            )
            .unwrap()
    }

    pub fn get_object_class(&self, obj: JObject) -> JClass {
        self.ptr.get_object_class(obj).unwrap()
    }

    pub fn string_to_jstring(&self, string: String) -> JString {
        self.ptr.new_string(string).unwrap()
    }

    pub fn clone_jstring_to_string(&self, string: JString) -> String {
        let chars = self.ptr.get_string_utf_chars(string).unwrap();

        // safe to unwrap cause we call GetStringUTFChars which guarantees that
        // UTF-8 encoding is used.
        let owned = unsafe { CStr::from_ptr(chars) }
            .to_str()
            .unwrap()
            .to_owned();
        self.ptr.release_string_utf_chars(string, chars).unwrap();

        owned
    }

    pub fn call_void_method(
        &self,
        object: JObject,
        method: JMethodID,
        args: &[JValue],
    ) {
        self.ptr
            .call_method_unchecked(
                object,
                method,
                JavaType::Primitive(Primitive::Void),
                args,
            )
            .unwrap();
    }

    pub fn exception_check(&self) -> bool {
        self.ptr.exception_check().unwrap()
    }

    pub fn exception_describe(&self) {
        self.ptr.exception_describe().unwrap()
    }

    pub fn exception_clear(&self) {
        self.ptr.exception_clear().unwrap()
    }
}

#[repr(transparent)]
pub struct JForeignObjectsArray<T> {
    _inner: jobjectArray,
    _marker: PhantomData<T>,
}

impl<T> JForeignObjectsArray<T> {
    pub fn jni_invalid_value() -> Self {
        Self {
            _inner: ptr::null_mut(),
            _marker: PhantomData,
        }
    }
}
