//! Wrapper for Dart exceptions.

use derive_more::Display;

/// Wrapper for Dart exception thrown when calling Dart code.
#[derive(Clone, Debug, Display, PartialEq)]
pub struct Error;
