use std::{cell::RefCell, os::raw, ptr};

use super::{super::JNI_VERSION, JNIEnv};

thread_local! {
    static THREAD_ATTACH_GUARD: RefCell<Option<AttachGuard >> = RefCell::new(None)
}

#[derive(Copy, Clone)]
pub struct JavaVM(*mut jni_sys::JavaVM);

/// Raw pointers are thread safe.
unsafe impl Send for JavaVM {}

/// Raw pointers are thread safe.
unsafe impl Sync for JavaVM {}

impl JavaVM {
    pub fn from_raw(ptr: *mut jni_sys::JavaVM) -> Self {
        if ptr.is_null() {
            panic!("null ptr")
        }
        JavaVM(ptr)
    }

    pub fn attach(&self) -> JNIEnv {
        match self.get_env() {
            Some(env) => env,
            None => self.attach_current_thread_impl(),
        }
    }

    fn get_env(&self) -> Option<JNIEnv> {
        let mut env = ptr::null_mut();
        let res = unsafe {
            (**self.0).GetEnv.unwrap()(
                self.0,
                (&mut env) as *mut *mut jni_sys::JNIEnv
                    as *mut *mut raw::c_void,
                JNI_VERSION,
            )
        };
        if res == 0 {
            Some(unsafe { JNIEnv::from_raw(env) })
        } else {
            None
        }
    }

    fn attach_current_thread_impl(&self) -> JNIEnv {
        let guard = AttachGuard::new(self.0);
        let env_ptr = unsafe { guard.attach_current_thread() };
        let env = unsafe { JNIEnv::from_raw(env_ptr as *mut jni_sys::JNIEnv) };

        THREAD_ATTACH_GUARD.with(move |f| {
            *f.borrow_mut() = Some(guard);
        });

        env
    }
}

#[derive(Debug)]
struct AttachGuard(*mut jni_sys::JavaVM);

impl AttachGuard {
    fn new(java_vm: *mut jni_sys::JavaVM) -> Self {
        Self(java_vm)
    }

    unsafe fn attach_current_thread(&self) -> *mut JNIEnv {
        let mut env_ptr = ptr::null_mut();
        let res = (**self.0).AttachCurrentThread.unwrap()(
            self.0,
            &mut env_ptr,
            ptr::null_mut(),
        );

        if res != 0 {
            panic!("failed to AttachCurrentThread")
        }

        env_ptr as *mut JNIEnv
    }
}

impl Drop for AttachGuard {
    fn drop(&mut self) {
        log::info!("DetachCurrentThread is called");
        let res = unsafe { (**self.0).DetachCurrentThread.unwrap()(self.0) };
        if res != 0 {
            panic!("failed to DetachCurrentThread");
        }
    }
}
