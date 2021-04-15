pub mod array;
pub mod dart_future;
pub mod executor;
pub mod option;

pub use self::array::Array;

use std::ffi::{CStr, CString};

pub unsafe fn from_dart_string(string: *const libc::c_char) -> String {
    CStr::from_ptr(string).to_str().unwrap().to_owned()
}

pub unsafe fn into_dart_string(string: String) -> *const libc::c_char {
    CString::new(string).unwrap().into_raw()
}
