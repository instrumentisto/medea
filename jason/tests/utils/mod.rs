#![cfg(target_arch = "wasm32")]
use futures::Future;
use js_sys::Array;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::MediaStream;

use medea_jason::utils::copy_js_ref;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn copy_js_ref() {
    let stream: MediaStream = MediaStream::new().unwrap();

    assert_ne!(stream.id(), stream.clone().id());
    assert_eq!(stream.id(), copy_js_ref(&stream).id());
}
