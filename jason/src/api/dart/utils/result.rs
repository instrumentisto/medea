use crate::api::dart::{utils::DartError, DartValue};

/// Dart structure which represents [`Result`] for the Dart error.
#[repr(u8)]
pub enum DartResult {
    Ok(DartValue),
    Err(DartError),
}

impl<T> From<Result<T, DartError>> for DartResult
where
    T: Into<DartValue>,
{
    fn from(res: Result<T, DartError>) -> Self {
        match res {
            Ok(val) => Self::Ok(val.into()),
            Err(e) => Self::Err(e),
        }
    }
}
