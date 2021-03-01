//! Helpers for application errors.

use std::{
    fmt::{Debug, Display},
    rc::Rc,
};

use derive_more::{Display, From};
use tracerr::{Trace, Traced};

use crate::platform;

pub use medea_macro::JsCaused;

/// Representation of an error which can caused by error returned from the
/// JS side.
pub trait JsCaused {
    /// Type of wrapper for JS error.
    type Error;

    /// Returns name of error.
    fn name(&self) -> &'static str;

    /// Returns JS error if it is the cause.
    fn js_cause(self) -> Option<Self::Error>;
}

// TODO: Consider moving to api::wasm.
/// Abstract application error.
#[derive(Clone, Debug, Display)]
#[display(fmt = "{}: {}\n{}", name, message, trace)]
pub struct JasonError {
    name: &'static str,
    message: String,
    trace: Trace,
    source: Option<platform::Error>,
}

impl JasonError {
    /// Returns name of error.
    pub fn name(&self) -> String {
        String::from(self.name)
    }

    /// Returns message of errors.
    pub fn message(&self) -> String {
        self.message.clone()
    }

    /// Returns trace information of error.
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }

    /// Returns JS side error if it the cause.
    pub fn source(&self) -> Option<platform::Error> {
        Clone::clone(&self.source)
    }

    /// Prints error information to default logger with `ERROR` level.
    pub fn print(&self) {
        log::error!("{}", self);
    }
}

impl<E: JsCaused + Display> From<(E, Trace)> for JasonError
where
    E::Error: Into<platform::Error>,
{
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
    fn from(traced: Traced<E>) -> Self {
        Self::from(traced.into_parts())
    }
}

/// Occurs if referenced value was dropped.
#[derive(Debug, Display, JsCaused)]
#[js(error = "platform::Error")]
#[display(fmt = "Handler is in detached state.")]
pub struct HandlerDetachedError;

/// Wrapper for [`serde_json::error::Error`] that provides [`Clone`], [`Debug`],
/// [`Display`] implementations.
#[derive(Clone, Debug, Display, From)]
#[from(forward)]
pub struct JsonParseError(Rc<serde_json::error::Error>);

impl PartialEq for JsonParseError {
    fn eq(&self, other: &Self) -> bool {
        self.0.line() == other.0.line()
            && self.0.column() == other.0.column()
            && self.0.classify() == other.0.classify()
    }
}
