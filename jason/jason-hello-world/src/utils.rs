use std::{
    ffi::{CStr, CString},
    slice,
};

#[repr(C)]
pub struct Array<T> {
    pub len: u64,
    pub arr: *const *mut T,
}

impl<T> Array<T> {
    pub fn new(arr: Vec<T>) -> Self {
        let out: Vec<_> = arr
            .into_iter()
            .map(|e| Box::into_raw(Box::new(e)))
            .collect();
        Self {
            len: out.len() as u64,
            arr: Box::leak(out.into_boxed_slice()).as_ptr(),
        }
    }
}

impl<T> Drop for Array<T> {
    fn drop(&mut self) {
        unsafe {
            slice::from_raw_parts_mut(self.arr as *mut i64, self.len as usize);
        };
    }
}

pub unsafe fn from_dart_string(string: *const libc::c_char) -> String {
    CStr::from_ptr(string).to_str().unwrap().to_owned()
}

pub unsafe fn into_dart_string(string: String) -> *const libc::c_char {
    CString::new(string).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn free_rust_string(s: *mut libc::c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}
