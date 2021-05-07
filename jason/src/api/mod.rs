//! External [`Jason`] API.

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {
        // TODO: make it private
        pub mod dart;
        pub use self::dart::*;
    } else {
        mod wasm;
        pub use self::wasm::*;
    }
}
