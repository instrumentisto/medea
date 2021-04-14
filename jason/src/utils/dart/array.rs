use dart_sys::{Dart_Handle, _Dart_Handle};
use std::{marker::PhantomData, slice};

#[repr(C)]
pub struct Array<T> {
    pub len: u64,
    pub arr: *const *mut T,
}

impl<T> From<Vec<T>> for Array<T> {
    fn from(arr: Vec<T>) -> Self {
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

// impl<T> From<Array<T>> for Vec<Box<T>> {
//     fn from(arr: Array<T>) -> Self {
//         let s = unsafe { slice::from_raw_parts_mut(arr.arr as *mut T, arr.len
// as usize) };         let mut out = Vec::with_capacity(arr.len as usize);
//         for p in s {
//             out.push(unsafe { Box::from_raw(p) });
//         }
//         out
//     }
// }

impl<T> Drop for Array<T> {
    fn drop(&mut self) {
        unsafe {
            slice::from_raw_parts_mut(self.arr as *mut i64, self.len as usize);
        }
    }
}

#[repr(C)]
pub struct DartHandleArray<T> {
    arr: Dart_Handle,
    _ty: PhantomData<T>,
}

impl<T> From<Dart_Handle> for DartHandleArray<T> {
    fn from(handle: Dart_Handle) -> Self {
        Self {
            arr: handle,
            _ty: PhantomData::default(),
        }
    }
}
