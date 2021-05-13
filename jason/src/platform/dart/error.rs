//! Wrapper for Dart exceptions.

use std::fmt;

use dart_sys::Dart_Handle;

/// Wrapper for Dart exception thrown when calling Dart code.
#[derive(Clone, Debug, PartialEq)]
pub struct Error(Dart_Handle);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DartPlatformError")
    }
}
