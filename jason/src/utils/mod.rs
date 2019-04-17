mod errors;

use web_sys::Window;

pub use self::errors::WasmErr;

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
