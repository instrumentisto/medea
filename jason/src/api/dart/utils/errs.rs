use std::ptr::NonNull;

use libc::c_char;

use dart_sys::Dart_Handle;

use crate::api::dart::utils::string_into_c_str;

type NewHandlerDetachedErrorCaller =
    extern "C" fn(NonNull<c_char>) -> Dart_Handle;

static mut NEW_HANDLER_DETACHED_ERROR_CALLER: Option<
    NewHandlerDetachedErrorCaller,
> = None;

#[repr(transparent)]
pub struct DartError(Dart_Handle);

#[no_mangle]
pub unsafe extern "C" fn register_new_completer_caller(
    f: NewHandlerDetachedErrorCaller,
) {
    NEW_HANDLER_DETACHED_ERROR_CALLER = Some(f);
}

#[must_use]
pub unsafe fn new_handler_detached_error(stacktrace: String) -> DartError {
    DartError(NEW_HANDLER_DETACHED_ERROR_CALLER.unwrap()(
        string_into_c_str(stacktrace),
    ))
}
