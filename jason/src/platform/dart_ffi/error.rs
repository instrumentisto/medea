//! More convenient wrapper for [`js_sys::Error`].

use std::borrow::Cow;

use derive_more::Display;

/// Wrapper for JS value which returned from JS side as error.
#[derive(Clone, Debug, Display, PartialEq)]
#[display(fmt = "{}: {}", name, message)]
pub struct Error {
    /// Name of JS error.
    pub name: Cow<'static, str>,

    /// Message of JS error.
    pub message: Cow<'static, str>,

    /// Original JS error.
    pub sys_cause: Option<dart_sys::Dart_Handle>,
}
