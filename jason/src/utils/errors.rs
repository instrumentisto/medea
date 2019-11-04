use std::borrow::Cow;

use thiserror::*;
use wasm_bindgen::{JsCast, JsValue};

/// Wrapper for JS value which returned from JS side as error.
#[derive(Error, Debug)]
#[error("{0}")]
pub struct WasmErr(Cow<'static, str>);

impl From<JsValue> for WasmErr {
    fn from(val: JsValue) -> Self {
        match val.dyn_into::<js_sys::Error>() {
            Ok(err) => String::from(err.to_string()).into(),
            Err(val) => match val.as_string() {
                Some(reason) => reason.into(),
                None => "no str representation for JsError".into(),
            },
        }
    }
}

impl From<&'static str> for WasmErr {
    fn from(msg: &'static str) -> Self {
        Self(Cow::Borrowed(msg))
    }
}

impl From<String> for WasmErr {
    fn from(msg: String) -> Self {
        Self(Cow::Owned(msg))
    }
}

/// Prints to console.error.
macro_rules! error {
    ($e:expr) => {
        web_sys::console::error_1(&$e.into())
    };
}
