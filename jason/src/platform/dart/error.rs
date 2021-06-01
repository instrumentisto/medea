//! Wrapper for Dart exceptions.

use std::{fmt, rc::Rc};

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use super::utils::dart_api::{
    Dart_DeletePersistentHandle_DL_Trampolined,
    Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

/// Wrapper for Dart exception thrown when calling Dart code.
#[derive(Clone, Debug, PartialEq)]
pub struct Error(Rc<Dart_PersistentHandle>);

impl Error {
    /// Returns [`Dart_Handle`] to the underlying error.
    #[inline]
    #[must_use]
    pub fn get_handle(&self) -> Dart_Handle {
        // We dont expose inner Dart_PersistentHandle anywhere, so we are sure
        // that it is valid at this point thus this should be safe.
        unsafe { Dart_HandleFromPersistent_DL_Trampolined(*self.0) }
    }
}

impl From<Dart_Handle> for Error {
    #[inline]
    fn from(err: Dart_Handle) -> Self {
        Self(Rc::new(unsafe {
            Dart_NewPersistentHandle_DL_Trampolined(err)
        }))
    }
}

impl Drop for Error {
    #[inline]
    fn drop(&mut self) {
        if Rc::strong_count(&self.0) == 1 {
            unsafe { Dart_DeletePersistentHandle_DL_Trampolined(*self.0) }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DartPlatformError")
    }
}
