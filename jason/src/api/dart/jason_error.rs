//! App error exported to JS side.
// TODO: This is just a copy of wasm version, Rust to Dart error propagation
//       will be implemented later.

use std::fmt::{Debug, Display};

use derive_more::{Display, From};
use tracerr::{Trace, Traced};

use crate::{platform, utils::JsCaused};

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
