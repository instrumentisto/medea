//! Helper functionality for passing [`String`]s through FFI boundaries.

use std::ffi::{CStr, CString};

/// Constructs Rust [`String`] from provided raw C string.
///
/// # Panics
///
/// If provided slice UTF-8 validation fails.
///
/// # Safety
///
/// Same as for [`CStr::from_ptr`].
#[must_use]
pub unsafe fn c_str_into_string(string: *const libc::c_char) -> String {
    CStr::from_ptr(string).to_str().unwrap().to_owned()
}

/// Leaks the [`String`] returning raw C string that can be passed through FFI
/// boundaries.
///
/// The pointer which this function returns must be returned to Rust and
/// reconstituted using [`CString::from_raw`] to be properly deallocated.
///
/// # Panics
///
/// If the provided [`String`] contains an internal 0 byte.
#[must_use]
pub fn string_into_c_str(string: String) -> *const libc::c_char {
    CString::new(string).unwrap().into_raw()
}

/// Retakes ownership of a [`CString`] that was transferred to Dart via
/// [`CString::into_raw`].
///
/// # Safety
///
/// Same as for [`CString::from_raw`].
#[no_mangle]
pub unsafe extern "C" fn String_free(s: *mut libc::c_char) {
    CString::from_raw(s);
}
