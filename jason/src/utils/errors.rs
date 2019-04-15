use std::error::Error;
use std::fmt::{Display, Formatter};
use wasm_bindgen::JsValue;
use web_sys::{console, CloseEvent, Event, MessageEvent, WebSocket};

pub enum WasmErr {
    JsError(JsValue),
    RustError(Box<dyn Error>),
    NoneError(&'static str),
    Other(String),
}

impl WasmErr {
    pub fn log_err(&self) {
        console::error_1(&JsValue::from_str(&format!("{}", self)));
    }
}

impl Display for WasmErr {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            WasmErr::JsError(val) => match val.as_string() {
                Some(reason) => write!(f, "{}", reason),
                None => write!(f, "{}", "no str representation for JsError"),
            },
            WasmErr::RustError(rust_err) => rust_err.fmt(f),
            WasmErr::NoneError(reason) => write!(f, "{}", reason),
            WasmErr::Other(reason) => write!(f, "{}", reason),
        }
    }
}

impl From<JsValue> for WasmErr {
    fn from(val: JsValue) -> Self {
        WasmErr::JsError(val)
    }
}

impl Into<JsValue> for WasmErr {
    fn into(self) -> JsValue {
        match self {
            WasmErr::JsError(value) => value,
            WasmErr::RustError(err) => JsValue::from_str(&format!("{}", err)),
            WasmErr::NoneError(reason) => JsValue::from_str(reason),
            WasmErr::Other(reason) => JsValue::from_str(&reason),
        }
    }
}

macro_rules! impl_from_error {
    ($error:ty) => {
        impl From<$error> for WasmErr {
            fn from(error: $error) -> Self {
                WasmErr::Other(format!("{}", error))
            }
        }
    };
}

impl_from_error!(std::cell::BorrowError);

// impl From<std::cell::BorrowError> for WasmErr {
//    fn from(val: JsValue) -> Self {
//        WasmErr::JsError(val)
//    }
//}
