use std::ptr;

use dart_sys::Dart_Handle;
use derive_more::{From, Into};
use libc::c_char;

use crate::{
    api::dart::{utils::string_into_c_str, DartValue},
    platform::{
        self, utils::dart_api::Dart_HandleFromPersistent_DL_Trampolined,
    },
};

type NewHandlerDetachedErrorCaller =
    extern "C" fn(ptr::NonNull<c_char>) -> Dart_Handle;

type NewMediaManagerExceptionCaller = extern "C" fn(
    msg: ptr::NonNull<c_char>,
    cause: DartValue,
    trace: ptr::NonNull<c_char>,
) -> Dart_Handle;

type NewArgumentErrorCaller =
    extern "C" fn(ptr::NonNull<c_char>) -> Dart_Handle;

// (Pointer<Utf8> message, Object? cause, Pointer<Utf8> nativeStackTrace)
static mut NEW_HANDLER_DETACHED_ERROR_CALLER: Option<
    NewHandlerDetachedErrorCaller,
> = None;

static mut NEW_MEDIA_MANAGER_EXCEPTION_CALLER: Option<
    NewMediaManagerExceptionCaller,
> = None;

static mut NEW_ARGUMENT_ERROR_CALLER: Option<NewArgumentErrorCaller> = None;

#[derive(Into)]
#[repr(transparent)]
pub struct DartError(ptr::NonNull<Dart_Handle>);

impl DartError {
    fn new(handle: Dart_Handle) -> DartError {
        DartError(ptr::NonNull::from(Box::leak(Box::new(handle))))
    }
}

impl From<platform::Error> for DartError {
    fn from(err: platform::Error) -> Self {
        Self::new(unsafe { Dart_HandleFromPersistent_DL_Trampolined(err.0) })
    }
}

#[no_mangle]
pub unsafe extern "C" fn register_new_handler_detached_error_caller(
    f: NewHandlerDetachedErrorCaller,
) {
    NEW_HANDLER_DETACHED_ERROR_CALLER = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_new_media_manager_exception_caller(
    f: NewMediaManagerExceptionCaller,
) {
    NEW_MEDIA_MANAGER_EXCEPTION_CALLER = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_new_argument_error_caller(
    f: NewArgumentErrorCaller,
) {
    NEW_ARGUMENT_ERROR_CALLER = Some(f);
}

#[must_use]
pub unsafe fn new_handler_detached_error(stacktrace: String) -> DartError {
    DartError::new(NEW_HANDLER_DETACHED_ERROR_CALLER.unwrap()(
        string_into_c_str(stacktrace),
    ))
}

#[must_use]
pub unsafe fn new_media_manager_exception(
    msg: String,
    cause: Option<platform::Error>,
    trace: String,
) -> DartError {
    DartError::new(NEW_MEDIA_MANAGER_EXCEPTION_CALLER.unwrap()(
        string_into_c_str(msg),
        cause.map(DartError::from).into(),
        string_into_c_str(trace),
    ))
}

#[derive(From)]
#[from(forward)]
pub struct ArgumentError(String);

impl From<ArgumentError> for DartError {
    fn from(err: ArgumentError) -> Self {
        unsafe {
            DartError::new(NEW_ARGUMENT_ERROR_CALLER.unwrap()(
                string_into_c_str(err.0),
            ))
        }
    }
}
