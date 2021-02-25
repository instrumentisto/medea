use derive_more::From;

use wasm_bindgen::prelude::*;

use crate::core;

#[wasm_bindgen]
#[derive(From)]
pub struct JasonError(core::utils::JasonError);

#[wasm_bindgen]
impl JasonError {
    /// Returns name of error.
    pub fn name(&self) -> String {
        self.0.name()
    }

    /// Returns message of errors.
    pub fn message(&self) -> String {
        self.0.message()
    }

    /// Returns trace information of error.
    pub fn trace(&self) -> String {
        self.0.trace()
    }

    /// Returns JS side error if it the cause.
    pub fn source(&self) -> Option<js_sys::Error> {
        self.0.source()
    }
}
