use std::borrow::Cow;

use derive_more::Display;
use js_caused::JsCaused;
use tracerr::Trace;
use wasm_bindgen::{prelude::*, JsCast};

/// Wrapper for JS value which returned from JS side as error.
#[derive(Debug, Display)]
#[display(fmt = "{}: {}", name, message)]
pub struct JsError {
    /// Name of JS error.
    name: Cow<'static, str>,

    /// Message of JS error.
    message: Cow<'static, str>,
}

impl From<JsValue> for JsError {
    fn from(val: JsValue) -> Self {
        match val.dyn_into::<js_sys::Error>() {
            Ok(err) => Self {
                name: Cow::Owned(err.name().into()),
                message: Cow::Owned(err.message().into()),
            },
            Err(val) => match val.as_string() {
                Some(reason) => Self {
                    name: Cow::from("Unknown error"),
                    message: Cow::from(reason),
                },
                None => Self {
                    name: Cow::from("Unknown error"),
                    message: Cow::from("no str representation for JsError"),
                },
            },
        }
    }
}

impl From<&JsError> for js_sys::Error {
    fn from(err: &JsError) -> Self {
        let error = Self::new(&err.message);
        error.set_name(&err.name);
        error
    }
}

/// Representation of app error exported to JS side.
///
/// Contains JS side error if it the cause and trace information.
#[wasm_bindgen]
#[derive(Debug, Display)]
#[display(fmt = "{}: {}\n{}", name, message, trace)]
pub struct JasonError {
    name: &'static str,
    message: String,
    trace: Trace,
    source: Option<js_sys::Error>,
}

#[wasm_bindgen]
impl JasonError {
    /// REturns name of error.
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
    pub fn source(&self) -> JsValue {
        self.source
            .as_ref()
            .map_or(JsValue::undefined(), Into::into)
    }
}

impl<E: JsCaused> From<(E, Trace)> for JasonError {
    fn from((err, trace): (E, Trace)) -> Self {
        let message = err.to_string();
        match err.js_cause() {
            Some(e) => Self {
                name: err.name(),
                message,
                trace,
                source: Some(e),
            },
            None => Self {
                name: err.name(),
                message,
                trace,
                source: None,
            },
        }
    }
}

/// Prints `$e` as `console.error()`.
macro_rules! console_error {
    ($e:expr) => {
        web_sys::console::error_1(&$e.into())
    };
}
