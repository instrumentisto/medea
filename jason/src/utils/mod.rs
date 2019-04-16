mod errors;

pub use self::errors::WasmErr;

use wasm_bindgen::{
    closure::Closure,
    convert::{FromWasmAbi, ReturnWasmAbi},
    prelude::*,
    JsCast,
};
use web_sys::{EventTarget, Window};

pub struct IntervalHandle(pub i32);

pub fn window() -> Window {
    // cannot use lazy_static since window is !Sync
    // safe to unwrap
    web_sys::window().unwrap()
}

impl Drop for IntervalHandle {
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.0);
    }
}
