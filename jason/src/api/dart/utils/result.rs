//! FFI-compatible [Result].

use crate::api::dart::{utils::DartError, DartValue};

/// FFI-compatible [Result].
#[repr(u8)]
pub enum DartResult {
    /// Contains the success [DartValue].
    Ok(DartValue),
    /// Contains the [DartError] value.
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
