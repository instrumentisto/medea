use std::{marker::PhantomData, slice};

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
