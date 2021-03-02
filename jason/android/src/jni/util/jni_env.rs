use std::{
    convert::TryFrom as _,
    ffi::{CStr, CString},
    marker::PhantomData,
    os::raw::c_char,
    ptr,
};

use jni_sys::{
    jclass, jlong, jmethodID, jobject, jobjectArray, jsize, jstring, jvalue,
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
        let field_id = <T>::jni_class_pointer_field();
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

    // TODO: create wrapper with DeleteGlobalRef on drop
    pub fn new_global_ref(&self, obj: jobject) -> jobject {
        unsafe { (**self.ptr).NewGlobalRef.unwrap()(self.ptr, obj) }
    }

    pub fn throw_new(&self, message: &str) {
        let exception_class = unsafe { JAVA_LANG_EXCEPTION };

        let res = unsafe {
            (**self.ptr).ThrowNew.unwrap()(
                self.ptr,
                exception_class,
                as_c_str_unchecked(message),
            )
        };
        if res != 0 {
            log::error!(
                "JNI ThrowNew({}) failed for class {:?} failed",
                message,
                exception_class
            );
        }
    }

    pub fn get_method_id(
        &self,
        class: jclass,
        name: &str,
        sig: &str,
    ) -> jmethodID {
        unsafe {
            (**self.ptr).GetMethodID.unwrap()(
                self.ptr,
                class,
                as_c_str_unchecked(name),
                as_c_str_unchecked(sig),
            )
        }
    }

    pub fn get_object_class(&self, obj: jobject) -> jclass {
        unsafe { (**self.ptr).GetObjectClass.unwrap()(self.ptr, obj) }
    }

    pub fn object_to_jobject<T: ForeignClass>(&self, rust_obj: T) -> jobject {
        let jcls = <T>::jni_class();
        assert!(!jcls.is_null());
        let field_id = <T>::jni_class_pointer_field();
        assert!(!field_id.is_null());
        let jobj: jobject =
            unsafe { (**self.ptr).AllocObject.unwrap()(self.ptr, jcls) };
        assert!(!jobj.is_null(), "object_to_jobject: AllocObject failed");
        let ret: jlong = <T>::box_object(rust_obj);
        unsafe {
            (**self.ptr).SetLongField.unwrap()(self.ptr, jobj, field_id, ret);
            if (**self.ptr).ExceptionCheck.unwrap()(self.ptr) != 0 {
                panic!(
                    "object_to_jobject: Can not set nativePtr field: catch \
                     exception"
                );
            }
        }

        jobj
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
            (**self.ptr).CallVoidMethod.unwrap()(
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

// Provided string MUST not contain null-bytes.
fn as_c_str_unchecked(string: &str) -> *const c_char {
    let null_terminated = &[string, "\0"].concat();
    null_terminated.as_ptr() as *const c_char
}
