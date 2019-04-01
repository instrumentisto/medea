mod utils;

use cfg_if::cfg_if;
use wasm_bindgen::prelude::*;

cfg_if::cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

#[wasm_bindgen]
pub struct Medea {
    token: String,
}

#[wasm_bindgen]
impl Medea {
    #[wasm_bindgen(constructor)]
    pub fn new(token: String) -> Self {
        Self { token }
    }

    pub fn get_token(&self) -> String {
        self.token.clone()
    }

    pub fn drop(self) {}
}
