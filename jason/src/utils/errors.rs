use std::{
    borrow::Cow,
    fmt::{Debug, Display},
    rc::Rc,
};

use derive_more::{Display, From};
use tracerr::{Trace, Traced};
use wasm_bindgen::{prelude::*, JsCast};

pub use medea_macro::JsCaused;

/// Prints provided message with [`Console.error()`].
///
/// May be useful during development.
///
/// __Unavailable in the release builds.__
///
/// [`Console.error()`]: https://tinyurl.com/psv3wqw
#[cfg(debug_assertions)]
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
#[derive(Clone, Debug, Display, PartialEq)]
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
#[derive(Clone, Debug, Display)]
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
        log::error!("{}", self);
    }
}

#[wasm_bindgen]
impl JasonError {
    /// Returns name of error.
    #[must_use]
    pub fn name(&self) -> String {
        String::from(self.name)
    }

    /// Returns message of errors.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    /// Returns trace information of error.
    #[must_use]
    pub fn trace(&self) -> String {
        self.trace.to_string()
    }

    /// Returns JS side error if it the cause.
    #[must_use]
    pub fn source(&self) -> Option<js_sys::Error> {
        Clone::clone(&self.source)
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
