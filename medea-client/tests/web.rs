#![cfg(target_arch = "wasm32")]

use medea_client::Medea;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn some_dummy_test() {
    let m = Medea::new("asd".to_string());
    assert_eq!("asd".to_string(), m.get_token());
}
