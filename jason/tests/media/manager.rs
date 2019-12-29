#![cfg(target_arch = "wasm32")]

use js_sys::Array;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use medea_jason::{
    media::MediaManager, AudioTrackConstraints, DeviceVideoTrackConstraints,
    MediaStreamConstraints,
};

use crate::{get_jason_error, MockNavigator};

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
    mock_navigator
        .error_enumerate_devices("failed_get_media_devices_info".into());
    let media_manager = MediaManager::default();
    let result =
        JsFuture::from(media_manager.new_handle().enumerate_devices()).await;
    mock_navigator.stop();
    match result {
        Ok(_) => assert!(false),
        Err(err) => {
            let e = get_jason_error(err);
            assert_eq!(e.name(), "EnumerateDevicesFailed");
            assert_eq!(
                e.message(),
                "MediaDevices.enumerateDevices() failed: Unknown JS error: \
                 failed_get_media_devices_info",
            );
        }
    }
}

#[wasm_bindgen_test]
async fn failed_get_user_media() {
    let mock_navigator = MockNavigator::new();
    mock_navigator.error_get_user_media("failed_get_user_media".into());
    let media_manager = MediaManager::default();
    let constraints = {
        let mut constraints = MediaStreamConstraints::new();
        let audio_constraints = AudioTrackConstraints::new();
        let video_constraints = DeviceVideoTrackConstraints::new();

        constraints.audio(audio_constraints);
        constraints.device_video(video_constraints);

        constraints
    };
    let result = JsFuture::from(
        media_manager.new_handle().init_local_stream(constraints),
    )
    .await;
    mock_navigator.stop();
    match result {
        Ok(_) => assert!(false),
        Err(err) => {
            let err = get_jason_error(err);
            assert_eq!(err.name(), "GetUserMediaFailed");
            assert_eq!(
                err.message(),
                "MediaDevices.getUserMedia() failed: Unknown JS error: \
                 failed_get_user_media",
            );
        }
    }
}

#[wasm_bindgen_test]
async fn failed_get_user_media2() {
    let mock_navigator = MockNavigator::new();

    let error = js_sys::Error::new("get_user_media_error_message");
    error.set_name("get_user_media_error_name");

    mock_navigator.error_get_user_media(error.into());
    let media_manager = MediaManager::default();
    let constraints = {
        let mut constraints = MediaStreamConstraints::new();
        let audio_constraints = AudioTrackConstraints::new();
        let video_constraints = DeviceVideoTrackConstraints::new();

        constraints.audio(audio_constraints);
        constraints.device_video(video_constraints);

        constraints
    };
    let result = JsFuture::from(
        media_manager.new_handle().init_local_stream(constraints),
    )
    .await;
    mock_navigator.stop();
    match result {
        Ok(_) => assert!(false),
        Err(err) => {
            let err = get_jason_error(err);
            assert_eq!(err.name(), "GetUserMediaFailed");
            assert_eq!(
                err.message(),
                "MediaDevices.getUserMedia() failed: \
                 get_user_media_error_name: get_user_media_error_message",
            );
        }
    }
}
