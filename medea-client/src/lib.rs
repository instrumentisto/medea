use wasm_bindgen::prelude::*;

// When the `console_error_panic_hook` feature is enabled, we can call the
// `set_panic_hook` function at least once during initialization, and then
// we will get better error messages if our code ever panics.
//
// For more details see
// https://github.com/rustwasm/console_error_panic_hook#readme
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct Medea {
    token: String,
}

#[wasm_bindgen]
impl Medea {
    #[wasm_bindgen(constructor)]
    pub fn new(token: String) -> Self {
        set_panic_hook();
        Self { token }
    }

    pub fn get_token(&self) -> String {
        self.token.clone()
    }

    pub fn drop(self) {}
}
