use std::ffi::{CStr, CString};

pub unsafe fn c_str_into_string(string: *const libc::c_char) -> String {
    CStr::from_ptr(string).to_str().unwrap().to_owned()
}

pub unsafe fn string_into_c_str(string: String) -> *const libc::c_char {
    CString::new(string).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn String_free(s: *mut libc::c_char) {
    CString::from_raw(s);
}
