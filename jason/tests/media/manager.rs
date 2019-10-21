#![cfg(target_arch = "wasm32")]

use js_sys::Array;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use medea_jason::media::MediaManager;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn get_media_devices_info() {
    let media_manager = MediaManager::default();
    let devices =
        JsFuture::from(media_manager.new_handle().enumerate_devices())
            .await
            .unwrap();

    let devices = Array::from(&devices);
    assert!(devices.length() >= 2);
}
