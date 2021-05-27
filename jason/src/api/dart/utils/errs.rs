use std::ptr;

use dart_sys::Dart_Handle;
use derive_more::{From, Into};
use libc::c_char;

use crate::api::dart::utils::string_into_c_str;

type NewArgumentErrorCaller =
    extern "C" fn(ptr::NonNull<c_char>) -> Dart_Handle;

static mut NEW_ARGUMENT_ERROR_CALLER: Option<NewArgumentErrorCaller> = None;

#[derive(Into)]
#[repr(transparent)]
pub struct DartError(ptr::NonNull<Dart_Handle>);

impl DartError {
    fn new(handle: Dart_Handle) -> DartError {
        DartError(ptr::NonNull::from(Box::leak(Box::new(handle))))
    }
}

#[no_mangle]
pub unsafe extern "C" fn register_new_argument_error_caller(
    f: NewArgumentErrorCaller,
) {
    NEW_ARGUMENT_ERROR_CALLER = Some(f);
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
