//! More convenient wrapper for [`js_sys::Error`].

use std::borrow::Cow;

use derive_more::Display;
use wasm_bindgen::{JsCast, JsValue};

use crate::platform;

/// Wrapper for JS value which returned from JS side as error.
#[derive(Clone, Debug, Display, PartialEq)]
#[display(fmt = "{}: {}", name, message)]
pub struct Error {
    /// Name of JS error.
    pub name: Cow<'static, str>,

    /// Message of JS error.
    pub message: Cow<'static, str>,

    /// Original JS error.
    pub sys_cause: Option<js_sys::Error>,
}

impl From<JsValue> for platform::Error {
    fn from(val: JsValue) -> Self {
        match val.dyn_into::<js_sys::Error>() {
            Ok(err) => Self {
                name: Cow::Owned(err.name().into()),
                message: Cow::Owned(err.message().into()),
                sys_cause: Some(err),
            },
            Err(val) => match val.as_string() {
                Some(reason) => Self {
                    name: "Unknown JS error".into(),
                    message: reason.into(),
                    sys_cause: None,
                },
                None => Self {
                    name: "Unknown JS error".into(),
                    message: format!("{:?}", val).into(),
                    sys_cause: None,
                },
            },
        }
    }
}
