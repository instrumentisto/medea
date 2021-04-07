use std::{any::Any, marker::PhantomData};

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use crate::{
    Dart_DeletePersistentHandle_DL_Trampolined,
    Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

pub type AnyClosureCaller = extern "C" fn(c: Dart_Handle, var: *mut dyn Any);

static mut ANY_CLOSURE_CALLER: Option<AnyClosureCaller> = None;

pub unsafe fn set_any_closure_caller(caller: AnyClosureCaller) {
    ANY_CLOSURE_CALLER = Some(caller);
}

pub struct DartCallback<T> {
    cb: Dart_PersistentHandle,
    _argument_type: PhantomData<T>,
}

unsafe impl<T> Send for DartCallback<T> {}

impl<T: 'static> DartCallback<T> {
    pub fn new(cb: Dart_Handle) -> Self {
        Self {
            cb: unsafe { Dart_NewPersistentHandle_DL_Trampolined(cb) },
            _argument_type: PhantomData::default(),
        }
    }

    pub fn call(&self, arg: T) {
        let closure_handle =
            unsafe { Dart_HandleFromPersistent_DL_Trampolined(self.cb) };
        unsafe {
            ANY_CLOSURE_CALLER.unwrap()(
                closure_handle,
                Box::into_raw(Box::new(arg) as Box<dyn Any>),
            );
        }
    }
}

impl<T> Drop for DartCallback<T> {
    fn drop(&mut self) {
        unsafe { Dart_DeletePersistentHandle_DL_Trampolined(self.cb) };
    }
}
