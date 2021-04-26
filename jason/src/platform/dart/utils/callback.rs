use dart_sys::Dart_Handle;

use crate::utils::dart::from_dart_string;

type VoidCallbackFunction = extern "C" fn(*mut VoidCallback) -> Dart_Handle;
static mut VOID_CALLBACK_FUNCTION: Option<VoidCallbackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_void_callback_function(
    f: VoidCallbackFunction,
) {
    VOID_CALLBACK_FUNCTION = Some(f);
}

pub struct VoidCallback(Box<dyn FnOnce()>);

impl VoidCallback {
    pub fn callback<F>(f: F) -> Dart_Handle
    where
        F: FnOnce() + 'static,
    {
        let this = Self(Box::new(f));
        unsafe {
            VOID_CALLBACK_FUNCTION.unwrap()(Box::into_raw(Box::new(this)))
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn call_string_callback(
    cb: *mut StringCallback,
    val: *const libc::c_char,
) {
    let cb = Box::from_raw(cb);
    cb.0(from_dart_string(val));
}
type StringCallbackFunction = extern "C" fn(*mut StringCallback) -> Dart_Handle;
static mut STRING_CALLBACK_FUNCTION: Option<StringCallbackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_string_callback_function(
    f: StringCallbackFunction,
) {
    STRING_CALLBACK_FUNCTION = Some(f);
}

pub struct StringCallback(Box<dyn FnOnce(String)>);

impl StringCallback {
    pub fn callback<F>(f: F) -> Dart_Handle
    where
        F: FnOnce(String) + 'static,
    {
        let this = Self(Box::new(f));
        unsafe {
            STRING_CALLBACK_FUNCTION.unwrap()(Box::into_raw(Box::new(this)))
        }
    }
}

type HandleMutCallbackFunction =
    extern "C" fn(*mut HandleMutCallback) -> Dart_Handle;
static mut HANDLE_MUT_CALLBACK_FUNCTION: Option<HandleMutCallbackFunction> =
    None;

#[no_mangle]
pub unsafe extern "C" fn call_handle_mut_callback(
    cb: *mut HandleMutCallback,
    val: Dart_Handle,
) {
    (*cb).0(val);
}
pub struct HandleMutCallback(Box<dyn FnMut(Dart_Handle)>);

impl HandleMutCallback {
    pub fn callback<F>(f: F) -> Dart_Handle
    where
        F: FnMut(Dart_Handle) + 'static,
    {
        let this = Self(Box::new(f));
        unsafe {
            HANDLE_MUT_CALLBACK_FUNCTION.unwrap()(Box::into_raw(Box::new(this)))
        }
    }
}

type HandleCallbackFunction = extern "C" fn(*mut HandleCallback) -> Dart_Handle;
static mut HANDLE_CALLBACK_FUNCTION: Option<HandleCallbackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn call_handle_callback(
    cb: *mut HandleCallback,
    handle: Dart_Handle,
) {
    let cb = Box::from_raw(cb);
    cb.0(handle);
}

pub struct HandleCallback(Box<dyn Fn(Dart_Handle)>);

impl HandleCallback {
    pub fn callback<F>(f: F) -> Dart_Handle
    where
        F: Fn(Dart_Handle) + 'static,
    {
        let this = Self(Box::new(f));
        unsafe {
            HANDLE_CALLBACK_FUNCTION.unwrap()(Box::into_raw(Box::new(this)))
        }
    }
}

type IntCallbackFunction = extern "C" fn(*mut IntCallback) -> Dart_Handle;
static mut INT_CALLBACK_FUNCTION: Option<IntCallbackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn call_int_callback(cb: *mut IntCallback, val: i32) {
    (*cb).0(val);
}

pub struct IntCallback(Box<dyn FnMut(i32)>);

impl IntCallback {
    pub fn callback<F>(f: F) -> Dart_Handle
    where
        F: FnMut(i32) + 'static,
    {
        let this = Self(Box::new(f));
        unsafe { INT_CALLBACK_FUNCTION.unwrap()(Box::into_raw(Box::new(this))) }
    }
}

type TwoArgCallbackFunction = extern "C" fn(*mut TwoArgCallback) -> Dart_Handle;
static mut TWO_ARG_CALLBACK_FUNCTION: Option<TwoArgCallbackFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn call_two_arg_callback(
    cb: *mut TwoArgCallback,
    first: Dart_Handle,
    second: Dart_Handle,
) {
    (*cb).0(first, second);
}

pub struct TwoArgCallback(Box<dyn FnMut(Dart_Handle, Dart_Handle)>);

impl TwoArgCallback {
    pub fn callback<F>(f: F) -> Dart_Handle
    where
        F: FnMut(Dart_Handle, Dart_Handle) + 'static,
    {
        let this = Self(Box::new(f));
        unsafe {
            TWO_ARG_CALLBACK_FUNCTION.unwrap()(Box::into_raw(Box::new(this)))
        }
    }
}
