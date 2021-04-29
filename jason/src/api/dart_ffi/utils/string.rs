//! Helper functionality for passing [`String`]s through FFI boundaries.

use std::ffi::{CStr, CString};

/// Helper
#[must_use]
pub unsafe fn c_str_into_string(string: *const libc::c_char) -> String {
    CStr::from_ptr(string).to_str().unwrap().to_owned()
}

#[must_use]
pub unsafe fn string_into_c_str(string: String) -> *const libc::c_char {
    CString::new(string).unwrap().into_raw()
}

/// Retakes ownership of a [`CString`] that was transferred to Dart via
/// [`CString::into_raw`].
///
/// # Safety
///
/// Same as of [`CString::from_raw`].
#[no_mangle]
pub unsafe extern "C" fn String_free(s: *mut libc::c_char) {
    CString::from_raw(s);
}
