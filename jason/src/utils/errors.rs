use std::{
    borrow::Cow,
    fmt::{Display, Formatter},
};

use medea_client_api_proto as proto;
use wasm_bindgen::JsValue;
use web_sys::console;

/// Generic application error.
#[derive(Debug)]
pub enum WasmErr {
    JsError(JsValue),
    Other(Cow<'static, str>),
}

impl WasmErr {
    // TODO:
    // 1. Send err to remote
    // probably should be possible only in debug build:
    // 2. Stacktrace?
    // 3. Medea state snapshot?
    pub fn log_err(&self) {
        console::error_1(&JsValue::from_str(&format!("{}", self)));
    }

    pub fn from_str<S>(msg: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        WasmErr::Other(msg.into())
    }
}

impl Display for WasmErr {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            WasmErr::JsError(val) => match val.as_string() {
                Some(reason) => write!(f, "{}", reason),
                None => write!(f, "no str representation for JsError"),
            },
            WasmErr::Other(reason) => write!(f, "{}", reason),
        }
    }
}

impl From<JsValue> for WasmErr {
    fn from(val: JsValue) -> Self {
        WasmErr::JsError(val)
    }
}

impl From<WasmErr> for JsValue {
    fn from(err: WasmErr) -> Self {
        match err {
            WasmErr::JsError(value) => value,
            WasmErr::Other(reason) => Self::from_str(&reason),
        }
    }
}

macro_rules! impl_from_error {
    ($error:ty) => {
        impl From<$error> for WasmErr {
            fn from(error: $error) -> Self {
                WasmErr::Other(format!("{}", error).into())
            }
        }
    };
}

impl_from_error!(std::cell::BorrowError);
impl_from_error!(serde_json::error::Error);
// TODO: improve macro to use generics
impl_from_error!(futures::sync::mpsc::SendError<proto::Event>);
