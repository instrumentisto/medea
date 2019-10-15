#![cfg(target_arch = "wasm32")]

use futures::Future;
use js_sys::Array;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use medea_jason::media::MediaManager;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test(async)]
fn get_media_devices_info() -> impl Future<Item = (), Error = JsValue> {
    let media_manager = MediaManager::default();
    JsFuture::from(media_manager.new_handle().enumerate_devices()).map(
        |devices_info| {
            let infos = Array::from(&devices_info);
            assert!(infos.length() >= 2);
        },
    )
}
