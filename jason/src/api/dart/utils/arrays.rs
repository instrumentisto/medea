//! Functionality for passing arrays from Rust to Dart.

use std::{ffi::c_void, marker::PhantomData, mem, ptr, slice};

use crate::api::ForeignClass;

/// Array of pointers to [`ForeignClass`] structs.
///
/// Can be safely returned from extern functions. Foreign code must manually
/// free this array by calling [`PtrArray_free()`].
#[repr(C)]
pub struct PtrArray<T = ()> {
    /// Pointer to the first element.
    ptr: *const *mut c_void,

    /// Array length.
    len: u64,

    /// Type of array elements.
    _element: PhantomData<T>,
}

impl<T: ForeignClass> PtrArray<T> {
    /// Constructs a new [`PtrArray`] from the provided iterator of
    /// [`ForeignClass`] structs. All elements are leaked and must be explicitly
    /// freed in the foreign code.
    pub fn new<I: IntoIterator<Item = T>>(arr: I) -> Self {
        let out: Vec<_> = arr.into_iter().map(ForeignClass::into_ptr).collect();
        Self {
            len: out.len() as u64,
            ptr: Box::leak(out.into_boxed_slice())
                .as_ptr()
                .cast::<*mut c_void>(),
            _element: PhantomData,
        }
    }
}

impl<T> PtrArray<T> {
    /// Erases type parameter on this [`PtrArray`].
    ///
    /// Intended to simplify things when passing [`PtrArray`] to Dart.
    ///
    /// Although this function drops `self` it won't clear an internal slice,
    /// its ownership is transferred to the resulting [`PtrArray`].
    #[inline]
    #[must_use]
    pub fn erase_type(mut self) -> PtrArray {
        PtrArray {
            ptr: mem::replace(&mut self.ptr, ptr::null()),
            len: self.len,
            _element: PhantomData,
        }
    }
}

impl<T> Drop for PtrArray<T> {
    /// Drops this [`PtrArray`].
    ///
    /// Clears the internal slice if its pointer is not null.
    ///
    /// # Safety
    ///
    /// Doesn't drop array elements. They are leaked and must be explicitly
    /// freed in the foreign code whenever foreign code doesn't need them.
    ///
    /// There is no relation between the lifetime of the items that array
    /// elements point to and this [`PtrArray`]'s lifetime thus drop order
    /// doesn't matter.
    #[allow(clippy::cast_possible_truncation)]
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

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
///
/// # Safety
///
/// Doesn't drop array elements. They are leaked and must be explicitly freed in
/// the foreign code whenever foreign code doesn't need them.
///
/// There is no relation between the lifetime of the items that array elements
/// point to and the [`PtrArray`]'s lifetime thus drop order doesn't matter.
///
/// This function should not be called before foreign code reads [`PtrArray`]
/// elements, otherwise pointers will be lost and data behind pointers will stay
/// leaked.
#[no_mangle]
pub unsafe extern "C" fn PtrArray_free(arr: PtrArray) {
    drop(arr);
}
