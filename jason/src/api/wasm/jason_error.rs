use std::{
    fmt::{Debug, Display},
};

use derive_more::From;
use derive_more::{Display};
use tracerr::{Trace, Traced};
use crate::utils::JsCaused;
use wasm_bindgen::prelude::*;

use crate::platform;

/// Representation of app error exported to JS side.
///
/// Contains JS side error if it the cause and trace information.
#[wasm_bindgen]
#[derive(From, Clone, Debug, Display)]
#[display(fmt = "{}: {}\n{}", name, message, trace)]
pub struct JasonError {
    name: &'static str,
    message: String,
    trace: Trace,
    source: Option<platform::Error>,
}

#[wasm_bindgen]
impl JasonError {
    /// Returns name of error.
    pub fn name(&self) -> String {
        self.name.to_string()
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
    pub fn source(&self) -> Option<js_sys::Error> {
        self.source.clone().and_then(|e| e.sys_cause)
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
