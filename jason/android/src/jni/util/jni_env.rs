use std::{
    convert::TryFrom as _,
    ffi::{CStr, CString},
    marker::PhantomData,
    os::raw::c_char,
    ptr,
};

use jni_sys::{
    jclass, jfieldID, jlong, jmethodID, jobject, jobjectArray, jsize, jstring,
    jvalue,
};

use crate::jni::{ForeignClass, JAVA_LANG_EXCEPTION};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JNIEnv<'a> {
    ptr: *mut jni_sys::JNIEnv,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> JNIEnv<'a> {
    pub unsafe fn from_raw(ptr: *mut jni_sys::JNIEnv) -> Self {
        assert!(!ptr.is_null());
        JNIEnv {
            ptr,
            _lifetime: PhantomData,
        }
    }

    pub fn get_static_object_field(
        &self,
        class: jclass,
        field: jfieldID,
    ) -> jobject {
        unsafe {
            (**self.ptr).GetStaticObjectField.unwrap()(self.ptr, class, field)
        }
    }

    pub fn set_long_field(&self, obj: jobject, field: jfieldID, value: jlong) {
        unsafe {
            (**self.ptr).SetLongField.unwrap()(self.ptr, obj, field, value);
        }
    }

    pub fn alloc_object(&self, class: jclass) -> jobject {
        unsafe { (**self.ptr).AllocObject.unwrap()(self.ptr, class) }
    }

    pub fn new_object_array<T: ForeignClass>(
        &self,
        mut arr: Vec<T>,
    ) -> JForeignObjectsArray<T> {
        let jcls: jclass = <T>::jni_class();
        assert!(!jcls.is_null());
        let arr_len = jsize::try_from(arr.len())
            .expect("invalid usize, in usize => to jsize conversation");
        let obj_arr: jobjectArray = unsafe {
            (**self.ptr).NewObjectArray.unwrap()(
                self.ptr,
                arr_len,
                jcls,
                ptr::null_mut(),
            )
        };
        assert!(!obj_arr.is_null());
        let field_id = <T>::native_ptr_field();
        assert!(!field_id.is_null());
        for (i, r_obj) in arr.drain(..).enumerate() {
            let jobj: jobject =
                unsafe { (**self.ptr).AllocObject.unwrap()(self.ptr, jcls) };
            assert!(!jobj.is_null());
            let r_obj: jlong = <T>::box_object(r_obj);
            unsafe {
                (**self.ptr).SetLongField.unwrap()(
                    self.ptr, jobj, field_id, r_obj,
                );
                if (**self.ptr).ExceptionCheck.unwrap()(self.ptr) != 0 {
                    panic!("Can not nativePtr field: catch exception");
                }
                (**self.ptr).SetObjectArrayElement.unwrap()(
                    self.ptr, obj_arr, i as jsize, jobj,
                );
                if (**self.ptr).ExceptionCheck.unwrap()(self.ptr) != 0 {
                    panic!("SetObjectArrayElement({}) failed", i);
                }
                (**self.ptr).DeleteLocalRef.unwrap()(self.ptr, jobj);
            }
        }
        JForeignObjectsArray {
            _inner: obj_arr,
            _marker: PhantomData,
        }
    }

    // TODO: create GlobalJObject wrapper with DeleteGlobalRef on drop
    pub fn new_global_ref(&self, obj: jobject) -> jobject {
        unsafe { (**self.ptr).NewGlobalRef.unwrap()(self.ptr, obj) }
    }

    pub fn throw_new(&self, message: *const c_char) {
        let exception_class = unsafe { JAVA_LANG_EXCEPTION };

        let res = unsafe {
            (**self.ptr).ThrowNew.unwrap()(self.ptr, exception_class, message)
        };
        if res != 0 {
            log::error!(
                "JNI ThrowNew failed for class {:?} failed",
                exception_class
            );
        }
    }

    pub fn get_method_id(
        &self,
        class: jclass,
        name: *const c_char,
        sig: *const c_char,
    ) -> jmethodID {
        unsafe { (**self.ptr).GetMethodID.unwrap()(self.ptr, class, name, sig) }
    }

    pub fn call_object_method(
        &self,
        object: jobject,
        method: jmethodID,
    ) -> jobject {
        unsafe {
            (**self.ptr).CallObjectMethod.unwrap()(self.ptr, object, method)
        }
    }

    pub fn get_object_class(&self, obj: jobject) -> jclass {
        unsafe { (**self.ptr).GetObjectClass.unwrap()(self.ptr, obj) }
    }

    pub fn string_to_jstring(&self, string: String) -> jstring {
        let string = string.into_bytes();
        unsafe {
            let string = CString::from_vec_unchecked(string);
            (**self.ptr).NewStringUTF.unwrap()(self.ptr, string.as_ptr())
        }
    }

    pub fn clone_jstring_to_string(&self, string: jstring) -> String {
        let chars = self.get_string_utf_chars(string);

        // safe to unwrap cause we call GetStringUTFChars which guarantees that
        // UTF-8 encoding is used.
        let owned = unsafe { CStr::from_ptr(chars) }
            .to_str()
            .unwrap()
            .to_owned();
        self.release_string_utf_chars(string, chars);

        owned
    }

    pub fn call_void_method(
        &self,
        object: jobject,
        method: jmethodID,
        args: &[jvalue],
    ) {
        unsafe {
            (**self.ptr).CallVoidMethodA.unwrap()(
                self.ptr,
                object,
                method,
                args.as_ptr(),
            );
        };
    }

    pub fn exception_check(&self) -> bool {
        let res = unsafe { (**self.ptr).ExceptionCheck.unwrap()(self.ptr) };

        res != 0
    }

    pub fn exception_describe(&self) {
        unsafe { (**self.ptr).ExceptionDescribe.unwrap()(self.ptr) }
    }

    pub fn exception_clear(&self) {
        unsafe { (**self.ptr).ExceptionClear.unwrap()(self.ptr) }
    }

    fn get_string_utf_chars(&self, string: jstring) -> *const c_char {
        unsafe {
            (**self.ptr).GetStringUTFChars.unwrap()(
                self.ptr,
                string,
                ptr::null_mut(),
            )
        }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn release_string_utf_chars(&self, string: jstring, chars: *const c_char) {
        unsafe {
            (**self.ptr).ReleaseStringUTFChars.unwrap()(self.ptr, string, chars)
        };
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
