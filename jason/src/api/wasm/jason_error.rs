//! App error exported to JS side.

use std::fmt::{Debug, Display};

use derive_more::{Display, From};
use tracerr::{Trace, Traced};
use wasm_bindgen::prelude::*;

use crate::{platform, utils::JsCaused};

/// Representation of an app error exported to JS side.
///
/// Contains JS side error if it's the cause, and a trace information.
#[wasm_bindgen]
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

#[wasm_bindgen]
impl JasonError {
    /// Returns a name of this error.
    #[must_use]
    pub fn name(&self) -> String {
        self.name.to_owned()
    }

    /// Returns a message of this error.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    /// Returns a trace information of this error.
    #[must_use]
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }

    /// Returns a JS side error if it's the cause.
    #[must_use]
    pub fn source(&self) -> Option<js_sys::Error> {
        self.source.clone().and_then(|e| e.sys_cause)
    }
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
