use std::{any::Any, marker::PhantomData};

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use super::trampoline::{
    Dart_DeletePersistentHandle_DL_Trampolined,
    Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

type PointerClosureCaller = extern "C" fn(c: Dart_Handle, var: *mut dyn Any);
static mut POINTER_CLOSURE_CALLER: Option<PointerClosureCaller> = None;
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub unsafe extern "C" fn register_pointer_closure_caller(
    caller: PointerClosureCaller,
) {
    POINTER_CLOSURE_CALLER = Some(caller);
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
        unsafe {
            let closure_handle =
                Dart_HandleFromPersistent_DL_Trampolined(self.cb);
            POINTER_CLOSURE_CALLER.unwrap()(
                closure_handle,
                Box::into_raw(Box::new(arg) as Box<dyn Any>),
            );
        }
    }
}

type UnitClosureCaller = extern "C" fn(Dart_Handle);
static mut UNIT_CLOSURE_CALLER: Option<UnitClosureCaller> = None;
#[no_mangle]
pub unsafe extern "C" fn register_unit_closure_caller(f: UnitClosureCaller) {
    UNIT_CLOSURE_CALLER = Some(f)
}

impl DartCallback<()> {
    pub fn call_unit(&self) {
        unsafe {
            let closure_handle =
                Dart_HandleFromPersistent_DL_Trampolined(self.cb);
            UNIT_CLOSURE_CALLER.unwrap()(closure_handle);
        }
    }
}

type IntClosureCaller = extern "C" fn(Dart_Handle, i32);
static mut INT_CLOSURE_CALLER: Option<IntClosureCaller> = None;
#[no_mangle]
pub unsafe extern "C" fn register_int_closure_caller(f: IntClosureCaller) {
    INT_CLOSURE_CALLER = Some(f);
}

impl DartCallback<i32> {
    pub fn call_int(&self, arg: i32) {
        unsafe {
            let closure_handle =
                Dart_HandleFromPersistent_DL_Trampolined(self.cb);
            INT_CLOSURE_CALLER.unwrap()(closure_handle, arg);
        }
    }
}

impl<T> Drop for DartCallback<T> {
    fn drop(&mut self) {
        unsafe { Dart_DeletePersistentHandle_DL_Trampolined(self.cb) };
    }
}
