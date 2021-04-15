use std::{cell::RefCell, marker::PhantomData};

use dart_sys::Dart_Handle;
use std::any::Any;

pub struct Callback<A>(RefCell<Option<Function<A>>>);

impl<A> Callback<A> {
    pub fn set_func(&self, f: Function<A>) {
        todo!()
    }

    pub fn is_set(&self) -> bool {
        todo!()
    }
}

// TODO: Maybe it's not needed
impl Callback<()> {
    pub fn call0(&self) {
        todo!()
    }
}

impl<A> Default for Callback<A> {
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}

pub struct Function<A> {
    handle: Dart_Handle,
    _ty: PhantomData<A>,
}

impl Function<()> {
    pub fn call0(&self) {
        todo!()
    }
}

type CallRustObjectFunction = extern "C" fn(Dart_Handle, *mut dyn Any);
static mut CALL_RUST_OBJECT_FUNCTION: Option<CallRustObjectFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_call_rust_object_function(
    f: CallRustObjectFunction,
) {
    CALL_RUST_OBJECT_FUNCTION = Some(f);
}

impl<A: 'static> Function<Box<A>> {
    pub fn call1(&self, arg: Box<A>) {
        let arg = Box::into_raw(arg);
        unsafe { CALL_RUST_OBJECT_FUNCTION.unwrap()(self.handle, arg) }
    }
}

type CallDartHandleFunction = extern "C" fn(Dart_Handle, Dart_Handle);
static mut CALL_DART_HANDLE_FUNCTION: Option<CallDartHandleFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_call_dart_handle_function(
    f: CallDartHandleFunction,
) {
    CALL_DART_HANDLE_FUNCTION = Some(f)
}

impl Function<Dart_Handle> {
    pub fn call_with_dart_handle(&self, arg: Dart_Handle) {
        unsafe {
            CALL_DART_HANDLE_FUNCTION.unwrap()(self.handle, arg);
        }
    }
}

type CallIntFunction = extern "C" fn(Dart_Handle, i32);
static mut CALL_INT_FUNCTION: Option<CallIntFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_call_int_function(
    f: CallIntFunction,
) {
    CALL_INT_FUNCTION = Some(f);
}

impl Function<i32> {
    pub fn call_with_int(&self, int: i32) {
        unsafe {
            CALL_INT_FUNCTION.unwrap()(self.handle, int);
        }
    }
}