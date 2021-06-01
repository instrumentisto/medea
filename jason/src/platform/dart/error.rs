//! Wrapper for Dart exceptions.

use std::fmt;

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use super::utils::dart_api::Dart_NewPersistentHandle_DL_Trampolined;

/// Wrapper for Dart exception thrown when calling Dart code.
#[derive(Clone, Debug, PartialEq)] // TODO: clone might be a problem
pub struct Error(pub Dart_PersistentHandle);

impl From<Dart_Handle> for Error {
    fn from(err: Dart_Handle) -> Self {
        Self(unsafe { Dart_NewPersistentHandle_DL_Trampolined(err) })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DartPlatformError")
    }
}
