//! App error exported to JS side.

use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::utils;

/// Representation of an app error exported to JS side.
///
/// Contains JS side error if it's the cause, and a trace information.
#[wasm_bindgen]
#[derive(From)]
pub struct JasonError(utils::JasonError);

#[wasm_bindgen]
impl JasonError {
    /// Returns a name of this error.
    pub fn name(&self) -> String {
        self.0.name()
    }

    /// Returns a message of this errors.
    pub fn message(&self) -> String {
        self.0.message()
    }

    /// Returns a trace information of this error.
    pub fn trace(&self) -> String {
        self.0.trace()
    }

    /// Returns a JS side error if it's the cause.
    pub fn source(&self) -> Option<js_sys::Error> {
        self.0.source().and_then(|a| a.sys_cause)
    }
}
