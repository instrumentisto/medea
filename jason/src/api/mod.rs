//! External [`Jason`] API.

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
        mod dart_ffi;
        pub use self::dart_ffi::*;
    } else {
        mod wasm;
        pub use self::wasm::*;
    }
}
