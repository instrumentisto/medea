use std::{
    borrow::Cow,
    fmt::{Debug, Display},
};

use derive_more::Display;
use tracerr::{Trace, Traced};
use wasm_bindgen::{prelude::*, JsCast};

pub use medea_macro::JsCaused;

/// Prints provided argument with `console.error()`.
pub fn console_error<M>(msg: M)
where
    M: Into<JsValue>,
{
    web_sys::console::error_1(&msg.into());
}

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

/// Wrapper for JS value which returned from JS side as error.
#[derive(Clone, Debug, Display)]
#[display(fmt = "{}: {}", name, message)]
pub struct JsError {
    /// Name of JS error.
    pub name: Cow<'static, str>,

    /// Message of JS error.
    pub message: Cow<'static, str>,
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
                    name: "Unknown JS error".into(),
                    message: reason.into(),
                },
                None => Self {
                    name: "Unknown JS error".into(),
                    message: format!("{:?}", val).into(),
                },
            },
        }
    }
}

impl From<JsError> for js_sys::Error {
    fn from(err: JsError) -> Self {
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

impl JasonError {
    /// Prints error information to `console.error()`.
    pub fn print(&self) {
        console_error(self.to_string());
    }
}

#[wasm_bindgen]
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
    pub fn source(&self) -> JsValue {
        self.source
            .as_ref()
            .map_or(JsValue::undefined(), Into::into)
    }
}

impl<E: JsCaused + Display> From<(E, Trace)> for JasonError
where
    E::Error: Into<js_sys::Error>,
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
    E::Error: Into<js_sys::Error>,
{
    fn from(traced: Traced<E>) -> Self {
        Self::from(traced.into_parts())
    }
}

/// Occurs if referenced value was dropped.
#[derive(Debug, Display, JsCaused)]
#[display(fmt = "Handler is in detached state.")]
pub struct HandlerDetachedError;
