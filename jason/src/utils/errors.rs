use std::{borrow::Cow, fmt};

use medea_client_api_proto as proto;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::console;

/// Generic application error.
#[derive(Debug, Clone)]
pub enum WasmErr {
    JsError(js_sys::Error),
    Custom(Cow<'static, str>),
    Untyped(JsValue),
}

impl WasmErr {
    // TODO: 1. Send err to remote
    //          probably should be possible only in debug build:
    //       2. Stacktrace?
    //       3. Medea state snapshot?
    pub fn log_err(&self) {
        console::error_1(&JsValue::from_str(&format!("{}", self)));
    }
}

impl From<&'static str> for WasmErr {
    fn from(msg: &'static str) -> Self {
        WasmErr::Custom(Cow::Borrowed(msg))
    }
}

impl From<String> for WasmErr {
    fn from(msg: String) -> Self {
        WasmErr::Custom(Cow::Owned(msg))
    }
}

impl From<JsValue> for WasmErr {
    fn from(val: JsValue) -> Self {
        match val.dyn_into::<js_sys::Error>() {
            Ok(err) => WasmErr::JsError(err),
            Err(val) => WasmErr::Untyped(val),
        }
    }
}

impl From<WasmErr> for JsValue {
    fn from(err: WasmErr) -> Self {
        match err {
            WasmErr::JsError(val) => val.into(),
            WasmErr::Custom(reason) => Self::from_str(&reason),
            WasmErr::Untyped(val) => val,
        }
    }
}

impl fmt::Display for WasmErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WasmErr::JsError(err) => {
                write!(f, "{}", String::from(err.to_string()))
            }
            WasmErr::Custom(reason) => write!(f, "{}", reason),
            WasmErr::Untyped(val) => match val.as_string() {
                Some(reason) => write!(f, "{}", reason),
                None => write!(f, "no str representation for JsError"),
            },
        }
    }
}

macro_rules! impl_from_error {
    ($error:ty) => {
        impl From<$error> for WasmErr {
            fn from(error: $error) -> Self {
                WasmErr::Custom(format!("{}", error).into())
            }
        }
    };
}

impl_from_error!(std::cell::BorrowError);
impl_from_error!(serde_json::error::Error);
// TODO: improve macro to use generics
impl_from_error!(futures::sync::mpsc::SendError<proto::Event>);
