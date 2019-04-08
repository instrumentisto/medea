use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::convert::{FromWasmAbi, ReturnWasmAbi};
use wasm_bindgen::JsCast;
use web_sys::{EventTarget};
use web_sys::Window;

pub fn bind_handler_fn_mut<F, A, R>(event: &str, target: &EventTarget, f: F) -> Result<Closure<dyn FnMut(A) -> R>, JsValue>
    where F: (FnMut(A) -> R) + 'static,
          A: FromWasmAbi + 'static,
          R: ReturnWasmAbi + 'static {
    let closure = Closure::wrap(Box::new(f) as Box<dyn FnMut(A) -> R>);
    target.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())?;
    Ok(closure)
}

pub fn bind_handler_fn_once<F, A, R>(event: &str, target: &EventTarget, f: F) -> Result<Closure<dyn FnMut(A) -> R>, JsValue>
    where F: (FnOnce(A) -> R) + 'static,
          A: FromWasmAbi + 'static,
          R: ReturnWasmAbi + 'static {
    let closure: Closure<FnMut(A) -> (R)> = Closure::once(f);
    target.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())?;
    Ok(closure)
}

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