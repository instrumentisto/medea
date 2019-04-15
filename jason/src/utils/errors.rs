use wasm_bindgen::JsValue;
use std::error::Error;
use crate::utils::errors::WasmErr::JsError;

pub enum WasmErr {
    JsError(JsValue),
    RustError(Box<dyn Error>),
    NoneError(&'static str)
}

impl From<wasm_bindgen::JsValue> for WasmErr {
    fn from(val: JsValue) -> Self {
        JsError(val)
    }
}
