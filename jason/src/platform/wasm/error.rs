//! More convenient wrapper for [`js_sys::Error`].

//! More convenient wrapper for [`js_sys::Error`].

use wasm_bindgen::{JsCast, JsValue};

pub use js_sys::Error;

/// Converts the provided [`JsValue`] to [`Error`].
#[inline]
#[must_use]
pub fn from(val: JsValue) -> Error {
    match val.dyn_into::<js_sys::Error>() {
        Ok(err) => err,
        Err(val) => match val.as_string() {
            Some(msg) => Error::new(&msg),
            None => Error::new(&format!("{:?}", val)),
        },
    }
}
