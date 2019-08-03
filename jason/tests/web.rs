#![cfg(target_arch = "wasm32")]

mod api;
mod peer;

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn some_dummy_test() {
    assert_eq!("asd", "asd");
}
