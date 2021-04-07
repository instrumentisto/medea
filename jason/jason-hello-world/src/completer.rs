use std::{any::Any, marker::PhantomData};

use dart_sys::Dart_Handle;

use crate::{
    callback::DartCallback, Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

pub type NewCompleter = extern "C" fn() -> Dart_Handle;
static mut NEW_COMPLETER: Option<NewCompleter> = None;

#[no_mangle]
pub unsafe extern "C" fn register_new_completer(f: NewCompleter) {
    NEW_COMPLETER = Some(f);
}

pub type CompleterCompleteCaller =
    extern "C" fn(c: Dart_Handle, var: *mut dyn Any);
static mut COMPLETER_COMPLETE_CALLER: Option<CompleterCompleteCaller> = None;

#[no_mangle]
pub unsafe extern "C" fn register_completer_complete(
    f: CompleterCompleteCaller,
) {
    COMPLETER_COMPLETE_CALLER = Some(f);
}

pub type CompleterCompleteErrorCaller =
    extern "C" fn(c: Dart_Handle, var: *mut dyn Any);
static mut COMPLETER_COMPLETE_ERROR_CALLER: Option<
    CompleterCompleteErrorCaller,
> = None;

#[no_mangle]
pub unsafe extern "C" fn register_completer_complete_error(
    f: CompleterCompleteErrorCaller,
) {
    COMPLETER_COMPLETE_ERROR_CALLER = Some(f);
}

pub type CompleterFutureCaller = extern "C" fn(c: Dart_Handle) -> Dart_Handle;
static mut COMPLETER_FUTURE_CALLER: Option<CompleterFutureCaller> = None;

#[no_mangle]
pub unsafe extern "C" fn register_completer_future(f: CompleterFutureCaller) {
    COMPLETER_FUTURE_CALLER = Some(f);
}

pub struct Completer<O, E> {
    handle: Dart_Handle,
    _success: PhantomData<O>,
    _error: PhantomData<E>,
}

impl<O: 'static, E: 'static> Completer<O, E> {
    pub fn new() -> Self {
        let completer;
        let persist;
        unsafe {
            completer = NEW_COMPLETER.unwrap()();
            persist = Dart_NewPersistentHandle_DL_Trampolined(completer);
        }
        Self {
            handle: persist,
            _success: PhantomData::default(),
            _error: PhantomData::default(),
        }
    }

    pub fn complete(&self, arg: O) {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            COMPLETER_COMPLETE_CALLER.unwrap()(
                handle,
                Box::into_raw(Box::new(arg) as Box<dyn Any>),
            );
        }
    }

    pub fn complete_error(&self, err: E) {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            COMPLETER_COMPLETE_ERROR_CALLER.unwrap()(
                handle,
                Box::into_raw(Box::new(err) as Box<dyn Any>),
            );
        }
    }

    pub fn future(&self) -> Dart_Handle {
        unsafe {
            let handle = Dart_HandleFromPersistent_DL_Trampolined(self.handle);
            COMPLETER_FUTURE_CALLER.unwrap()(handle)
        }
    }
}

unsafe impl<O, E> Send for Completer<O, E> {}
