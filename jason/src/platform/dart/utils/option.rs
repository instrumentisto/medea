use dart_sys::Dart_Handle;
use derive_more::From;

use crate::api::dart::utils::c_str_into_string;

type IsSomeFunction = extern "C" fn(Dart_Handle) -> i32;
static mut IS_SOME_FUNCTION: Option<IsSomeFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RustHandleOption__is_some(f: IsSomeFunction) {
    IS_SOME_FUNCTION = Some(f);
}

type GetFunction = extern "C" fn(Dart_Handle) -> Dart_Handle;
static mut GET_FUNCTION: Option<GetFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_RustHandleOption__get(f: GetFunction) {
    GET_FUNCTION = Some(f);
}

#[derive(From)]
pub struct RustHandleOption(Dart_Handle);

impl From<RustHandleOption> for Option<Dart_Handle> {
    fn from(from: RustHandleOption) -> Self {
        if unsafe { IS_SOME_FUNCTION.unwrap()(from.0) } == 1 {
            Some(unsafe { GET_FUNCTION.unwrap()(from.0) })
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct DartOption {
    is_some: i8,
    val: Dart_Handle,
}

impl From<DartOption> for Option<Dart_Handle> {
    fn from(from: DartOption) -> Self {
        if from.is_some == 1 {
            Some(from.val)
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct DartStringOption {
    is_some: i8,
    val: *const libc::c_char,
}

impl From<DartStringOption> for Option<String> {
    fn from(from: DartStringOption) -> Self {
        if from.is_some == 1 {
            unsafe { Some(c_str_into_string(from.val)) }
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct DartIntOption {
    is_some: i8,
    val: i32,
}

impl From<DartIntOption> for Option<i32> {
    fn from(from: DartIntOption) -> Self {
        if from.is_some == 1 {
            Some(from.val)
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct DartUIntOption {
    is_some: i8,
    val: u32,
}

impl From<DartUIntOption> for Option<u32> {
    fn from(from: DartUIntOption) -> Self {
        if from.is_some == 1 {
            Some(from.val)
        } else {
            None
        }
    }
}
