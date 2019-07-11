mod api;
mod rpc;
mod utils;

#[doc(inline)]
pub use self::api::{Jason, RoomHandle};

#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

#[cfg(not(feature = "console_error_panic_hook"))]
pub fn set_panic_hook() {}
