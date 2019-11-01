use std::borrow::Cow;

use anyhow::Error;
use thiserror::*;
use tracerr::{Trace, Traced};
use wasm_bindgen::{prelude::*, JsCast};

// Wrapper for JS value which returned from JS side as error.
// #[derive(Error, Debug)]
// #[error("{0}")]
// pub struct WasmErr(Cow<'static, str>);
//
// impl From<JsValue> for WasmErr {
// fn from(val: JsValue) -> Self {
// match val.dyn_into::<js_sys::Error>() {
// Ok(err) => String::from(err.to_string()).into(),
// Err(val) => match val.as_string() {
// Some(reason) => reason.into(),
// None => "no str representation for JsError".into(),
// },
// }
// }
// }
//
// impl From<&'static str> for WasmErr {
// fn from(msg: &'static str) -> Self {
// Self(Cow::Borrowed(msg))
// }
// }
//
// impl From<String> for WasmErr {
// fn from(msg: String) -> Self {
// Self(Cow::Owned(msg))
// }
// }

#[derive(Error, Debug)]
#[error("{name}: {message}")]
pub struct WasmErr {
    name: Cow<'static, str>,
    message: Cow<'static, str>,
}

impl From<JsValue> for WasmErr {
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

impl From<&WasmErr> for JsValue {
    fn from(err: &WasmErr) -> Self {
        let error = js_sys::Error::new(&err.message);
        error.set_name(&err.name);
        error.into()
    }
}

#[wasm_bindgen]
pub struct JasonError {
    name: &'static str,
    message: String,
    trace: Trace,
    source: Option<WasmErr>,
}

#[wasm_bindgen]
impl JasonError {
    pub fn name(&self) -> String {
        String::from(self.name)
    }

    pub fn message(&self) -> String {
        self.message.clone()
    }

    pub fn trace(&self) -> String {
        self.trace.to_string()
    }

    pub fn source(&self) -> JsValue {
        self.source
            .as_ref()
            .map_or(JsValue::undefined(), Into::into)
    }
}

impl From<Traced<Error>> for JasonError {
    fn from(error: Traced<Error>) -> Self {
        let (err, trace) = error.unwrap();
        let message = err.to_string();
        match err.downcast::<WasmErr>() {
            Ok(e) => Self {
                name: "Js error",
                message,
                trace,
                source: Some(e),
            },
            Err(_) => Self {
                name: "Internal error",
                message,
                trace,
                source: None,
            },
        }
    }
}
