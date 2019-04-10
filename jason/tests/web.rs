#![cfg(target_arch = "wasm32")]

use jason::Jason;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn some_dummy_test() {
    let m = Jason::new("asd".to_owned());
    assert_eq!("asd", &m.get_token());
}
