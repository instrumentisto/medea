//! FFI-compatible [`Result`] for Dart.

use crate::api::dart::{utils::DartError, DartValue};

/// FFI-compatible [`Result`] for Dart.
#[repr(u8)]
pub enum DartResult {
    /// Success [`DartValue`].
    Ok(DartValue),

    /// [`DartError`] value.
    Err(DartError),
}

impl<T: Into<DartValue>> From<Result<T, DartError>> for DartResult {
    #[inline]
    fn from(res: Result<T, DartError>) -> Self {
        match res {
            Ok(val) => Self::Ok(val.into()),
            Err(e) => Self::Err(e),
        }
    }
}

impl<T: Into<DartError>> From<T> for DartResult {
    #[inline]
    fn from(err: T) -> Self {
        DartResult::Err(err.into())
    }
}
