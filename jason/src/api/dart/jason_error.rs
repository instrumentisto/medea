//! App error exported to JS side.
// TODO: This is just a copy of wasm version, Rust to Dart error propagation
//       will be implemented later.

use std::{
    fmt::{Debug, Display},
    ptr,
};

use derive_more::{Display, From};
use libc::c_char;
use tracerr::{Trace, Traced};

use crate::{api::dart::utils::string_into_c_str, platform, utils::JsCaused};

use super::ForeignClass;

impl ForeignClass for JasonError {}

/// Representation of an app error exported to JS side.
///
/// Contains JS side error if it's the cause, and a trace information.
#[derive(From, Clone, Debug, Display)]
#[display(fmt = "{}: {}\n{}", name, message, trace)]
pub struct JasonError {
    /// Name of this [`JasonError`].
    name: &'static str,

    /// Message describing this [`JasonError`].
    message: String,

    /// [`Trace`] information of this [`JasonError`].
    trace: Trace,

    /// Optional cause of this [`JasonError`] as a JS side error.
    source: Option<platform::Error>,
}

impl<E: JsCaused + Display> From<(E, Trace)> for JasonError
where
    E::Error: Into<platform::Error>,
{
    #[inline]
    fn from((err, trace): (E, Trace)) -> Self {
        Self {
            name: err.name(),
            message: err.to_string(),
            trace,
            source: err.js_cause().map(Into::into),
        }
    }
}

impl<E: JsCaused + Display> From<Traced<E>> for JasonError
where
    E::Error: Into<platform::Error>,
{
    #[inline]
    fn from(traced: Traced<E>) -> Self {
        Self::from(traced.into_parts())
    }
}

/// Error representation for the Dart side.
#[repr(C)]
pub struct DartError {
    /// Name of this error.
    pub name: *const c_char,

    /// Message of this error.
    pub message: *const c_char,

    /// Stacktrace of this error.
    pub stacktrace: *const c_char,
}

impl DartError {
    /// Returns `null` [`DartError`].
    #[must_use]
    pub fn null() -> Self {
        Self {
            name: ptr::null(),
            message: ptr::null(),
            stacktrace: ptr::null(),
        }
    }
}

impl From<JasonError> for DartError {
    fn from(err: JasonError) -> Self {
        Self {
            name: string_into_c_str(err.name.to_string()).as_ptr(),
            message: string_into_c_str(err.message.clone()).as_ptr(),
            stacktrace: string_into_c_str(err.trace.to_string()).as_ptr(),
        }
    }
}
