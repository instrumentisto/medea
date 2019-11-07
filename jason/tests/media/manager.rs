#![cfg(target_arch = "wasm32")]

use futures::TryFutureExt;
use js_sys::Array;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use medea_jason::{
    media::MediaManager, utils::WasmErr, AudioTrackConstraints,
    MediaStreamConstraints, VideoTrackConstraints,
};

use crate::MockNavigator;

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

#[wasm_bindgen_test]
async fn failed_get_media_devices_info() {
    let mock_navigator = MockNavigator::new();
    mock_navigator.error_enumerate_devices("some error".into());
    let media_manager = MediaManager::default();
    let result = JsFuture::from(media_manager.new_handle().enumerate_devices())
        .map_err(WasmErr::from)
        .await;
    mock_navigator.stop();
    match result {
        Ok(_) => assert!(false),
        Err(err) => assert_eq!(
            err.to_string(),
            "Error: MediaDevices.enumerateDevices() failed: some error"
                .to_string()
        ),
    }
}

#[wasm_bindgen_test]
async fn failed_get_user_media() {
    let mock_navigator = MockNavigator::new();
    mock_navigator.error_get_user_media("some error".into());
    let media_manager = MediaManager::default();
    let constraints = {
        let mut constraints = MediaStreamConstraints::new();
        let audio_constraints = AudioTrackConstraints::new();
        let video_constraints = VideoTrackConstraints::new();

        constraints.audio(audio_constraints);
        constraints.video(video_constraints);

        constraints
    };
    let result = JsFuture::from(
        media_manager.new_handle().init_local_stream(constraints),
    )
    .map_err(WasmErr::from)
    .await;
    mock_navigator.stop();
    match result {
        Ok(_) => assert!(false),
        Err(err) => assert_eq!(
            err.to_string(),
            "Error: MediaDevices.getUserMedia() failed: some error".to_string()
        ),
    }
}
