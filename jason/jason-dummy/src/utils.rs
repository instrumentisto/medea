use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    slice,
};

#[repr(C)]
pub struct PtrArray<T = ()> {
    len: u64,
    arr: *const *mut (),
    marker: PhantomData<T>,
}

impl<T> PtrArray<T> {
    pub fn new(arr: Vec<T>) -> Self {
        let out: Vec<_> = arr
            .into_iter()
            .map(|e| Box::into_raw(Box::new(e)))
            .collect();
        Self {
            len: out.len() as u64,
            arr: Box::leak(out.into_boxed_slice()).as_ptr().cast::<*mut ()>(),
            marker: PhantomData::default(),
        }
    }
}

impl<T> Drop for PtrArray<T> {
    #[allow(clippy::cast_possible_truncation)]
    fn drop(&mut self) {
        // Only dropping boxed slice. Elements are leaked and must be
        // explicitly freed in foreign code.
        unsafe {
            let slice = slice::from_raw_parts_mut(
                self.arr as *mut *mut (),
                self.len as usize,
            );
            Box::from_raw(slice);
        };
    }
}

#[no_mangle]
pub unsafe extern "C" fn PtrArray_free(arr: PtrArray) {
    drop(arr);
}

pub unsafe fn c_str_into_string(string: *const libc::c_char) -> String {
    CStr::from_ptr(string).to_str().unwrap().to_owned()
}

pub unsafe fn string_into_c_str(string: String) -> *const libc::c_char {
    CString::new(string).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn String_free(s: *mut libc::c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}

pub unsafe fn ptr_from_dart_as_mut<'a, T>(ptr: *mut T) -> &'a mut T {
    match ptr.as_mut() {
        Some(reference) => reference,
        None => {
            unimplemented!()
        }
    }
}
