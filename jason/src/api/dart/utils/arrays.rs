//! Functionality for passing arrays from Rust to Dart.

use std::{marker::PhantomData, slice};

use super::super::ForeignClass;

/// Array of pointers to [`ForeignClass`] structs.
///
/// Can be safely returned from extern functions. Foreign code must manually
/// free this array by calling [`PtrArray_free`].
#[repr(C)]
pub struct PtrArray<T = ()> {
    /// Array length.
    len: u64,
    /// Pointer to the first element.
    arr: *const *mut (),

    /// Array elements type marker.
    _marker: PhantomData<T>,
}

impl<T: ForeignClass> PtrArray<T> {
    /// Constructs [`PtrArray`] from the provided iterator of [`ForeignClass`]
    /// structs. All elements are leaked and must be explicitly freed in the
    /// foreign code.
    pub fn new<I: IntoIterator<Item = T>>(arr: I) -> Self {
        let out: Vec<_> = arr.into_iter().map(ForeignClass::into_ptr).collect();
        Self {
            len: out.len() as u64,
            arr: Box::leak(out.into_boxed_slice()).as_ptr().cast::<*mut ()>(),
            _marker: PhantomData::default(),
        }
    }
}

impl<T> Drop for PtrArray<T> {
    #[allow(clippy::cast_possible_truncation)]
    fn drop(&mut self) {
        // Only dropping boxed slice. Elements are leaked and must be
        // explicitly freed in the foreign code.
        unsafe {
            let slice = slice::from_raw_parts_mut(
                self.arr as *mut *mut (),
                self.len as usize,
            );
            Box::from_raw(slice);
        };
    }
}

/// Drops the provided [`PtrArray`].
#[no_mangle]
pub unsafe extern "C" fn PtrArray_free(arr: PtrArray) {
    drop(arr);
}
