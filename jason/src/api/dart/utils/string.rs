//! Helper functionality for passing [`String`]s through FFI boundaries.

use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
};

/// Constructs a Rust [`String`] from the provided raw C string.
///
/// # Panics
///
/// If the provided C string UTF-8 validation fails.
///
/// # Safety
///
/// Same as for [`CStr::from_ptr()`].
#[inline]
#[must_use]
pub unsafe fn c_str_into_string(string: *const c_char) -> String {
    CStr::from_ptr(string).to_str().unwrap().to_owned()
}

/// Leaks the given [`String`] returning a raw C string that can be passed
/// through FFI boundaries.
///
/// The pointer (returned by this function) must be returned to Rust and
/// reconstituted via [`CString::from_raw()`] for proper deallocating.
///
/// # Panics
///
/// If the provided [`String`] contains an internal `0x0` byte.
#[inline]
#[must_use]
pub fn string_into_c_str(string: String) -> *const c_char {
    CString::new(string).unwrap().into_raw()
}

/// Retakes ownership over a [`CString`] previously transferred to Dart via
/// [`CString::into_raw()`].
///
/// # Safety
///
/// Same as for [`CString::from_raw()`].
#[no_mangle]
pub unsafe extern "C" fn String_free(s: *mut c_char) {
    CString::from_raw(s);
}
