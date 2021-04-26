use derive_more::Display;

use crate::platform::dart::utils::handle::DartHandle;

/// Wrapper for Dart value which returned from Dart side as error.
#[derive(Clone, Debug, Display, PartialEq)]
#[display(fmt = "{}: {}", name, message)]
pub struct Error {
    /// Name of JS error.
    pub name: String,

    /// Message of JS error.
    pub message: String,

    pub sys_cause: Option<DartHandle>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DartError(DartHandle);
