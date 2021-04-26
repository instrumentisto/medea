//! Functionality for passing arrays from Rust to Dart.

use std::{marker::PhantomData, slice};

use libc::c_void;

use crate::ForeignClass;

/// Array of pointers to [`ForeignClass`] structs.
///
/// Can be safely returned from extern functions. Foreign code must manually
/// free this array by calling [`PtrArray_free`].
#[repr(C)]
pub struct PtrArray<T = ()> {
    /// Pointer to the first element.
    ptr: *const *mut libc::c_void,

    /// Array length.
    len: u64,

    /// Array elements type marker.
    _element: PhantomData<T>,
}

impl<T: ForeignClass> PtrArray<T> {
    /// Constructs [`PtrArray`] from the provided iterator of [`ForeignClass`]
    /// structs. All elements are leaked and must be explicitly freed in the
    /// foreign code.
    pub fn new<I: IntoIterator<Item = T>>(arr: I) -> Self {
        let out: Vec<_> = arr.into_iter().map(ForeignClass::into_ptr).collect();
        Self {
            len: out.len() as u64,
            ptr: Box::leak(out.into_boxed_slice()).as_ptr().cast::<*mut c_void>(),
            _element: PhantomData,
        }
    }
}

impl<T> Drop for PtrArray<T> {
    #[allow(clippy::cast_possible_truncation)]
    fn drop(&mut self) {
        // Only dropping a boxed slice. Elements are leaked and must be
        // explicitly freed in the foreign code.
        unsafe {
            let slice = slice::from_raw_parts_mut(
                self.ptr as *mut *mut c_void,
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
