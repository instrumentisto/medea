//! Provides logging utilities, used by Jason.

use wasm_bindgen::JsValue;

/// Re-exports common definitions for logging.
///
/// Use this module as following:
/// ```rust
/// use medea_jason::log::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{log_debug, log_error};
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

/// Formats log message.
///
/// Use it same as [`std::format`] macro.
#[macro_export]
macro_rules! format_log {
    ($($arg:tt)*) => {
        format!("[{}:{}]: ", module_path!(), line!()) + &format!($($arg)*)
    };
}

/// Prints provided message same as [`format`] macro with a [`console_debug`]
/// function.
///
/// [`module_path`] and [`line`] will be added to the start of message.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::log::console_debug($crate::format_log!($($arg)*));
    };
}

/// Prints provided message same as [`format`] macro with a [`console_error`]
/// function.
///
/// [`module_path`] and [`line`] will be added to the start of message.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::log::console_error($crate::format_log!($($arg)*));
    };
}
