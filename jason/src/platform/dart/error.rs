use std::borrow::Cow;

use derive_more::Display;

/// Wrapper for Dart value which returned from Dart side as error.
#[derive(Clone, Debug, Display, PartialEq)]
#[display(fmt = "{}: {}", name, message)]
pub struct Error {
    /// Name of JS error.
    pub name: String,

    /// Message of JS error.
    pub message: String,
}
