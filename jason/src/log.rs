use wasm_bindgen::JsValue;

pub mod prelude {
    pub use crate::log_debug;
}

/// Prints provided message with [`Console.error()`].
///
/// [`Console.error()`]: https://tinyurl.com/psv3wqw
pub fn console_error<M>(msg: M)
where
    M: Into<JsValue>,
{
    web_sys::console::error_1(&msg.into());
}

/// Prints provided message with [`Console.debug()`].
pub fn console_debug<M>(msg: M)
where
    M: Into<JsValue>,
{
    web_sys::console::debug_1(&msg.into());
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::log::console_debug(&format!($($arg)*));
    };
}
