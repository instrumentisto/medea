#![cfg(target_arch = "wasm32")]

use std::iter::FromIterator;

use js_sys::Array as JsArray;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys as sys;

use medea_jason::{
    media::{MediaKind, MediaManager, MediaManagerError},
    AudioTrackConstraints, DeviceVideoTrackConstraints,
    DisplayVideoTrackConstraints, MediaStreamSettings,
};

use crate::{get_jason_error, is_firefox, MockNavigator};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn get_media_devices_info() {
    let media_manager = MediaManager::default();
    let devices =
        JsFuture::from(media_manager.new_handle().enumerate_devices())
            .await
            .unwrap();

    let devices = JsArray::from(&devices);
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
        let mut constraints = MediaStreamSettings::new();
        constraints.audio(AudioTrackConstraints::new());
        constraints.device_video(DeviceVideoTrackConstraints::new());
        constraints
    };
    let result = JsFuture::from(
        media_manager.new_handle().init_local_tracks(&constraints),
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
        let mut constraints = MediaStreamSettings::new();
        constraints.audio(AudioTrackConstraints::new());
        constraints.device_video(DeviceVideoTrackConstraints::new());
        constraints
    };
    let result = JsFuture::from(
        media_manager.new_handle().init_local_tracks(&constraints),
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

/// 1. Do `media_manager.get_stream(caps)`
/// 2. Only one `getUserMedia` request
/// 3. Do `media_manager.get_stream(caps)`
/// 4. Got same track, still one `getUserMedia` request
#[wasm_bindgen_test]
async fn same_track_for_same_constraints() {
    let mock_navigator = MockNavigator::new();

    let media_manager = MediaManager::default();
    let constraints = {
        let mut constraints = MediaStreamSettings::new();
        constraints.audio(AudioTrackConstraints::new());
        constraints
    };

    // first request
    let mut tracks =
        media_manager.get_tracks(constraints.clone()).await.unwrap();

    assert_eq!(tracks.len(), 1);
    let (track1, track1_is_new) = tracks.pop().unwrap();

    assert!(track1_is_new);
    assert_eq!(track1.kind(), MediaKind::Audio);
    assert_eq!(mock_navigator.get_user_media_requests_count(), 1);

    // second request, same track, no additional getUserMedia requests
    let mut tracks =
        media_manager.get_tracks(constraints.clone()).await.unwrap();

    assert_eq!(tracks.len(), 1);
    let (track2, track2_is_new) = tracks.pop().unwrap();

    assert!(!track2_is_new);
    assert_eq!(track1.id(), track2.id());
    assert_eq!(track2.kind(), MediaKind::Audio);
    assert_eq!(mock_navigator.get_user_media_requests_count(), 1);
}

/// 1. Do `media_manager.get_stream(caps)`
/// 2. Only one `getUserMedia` request
/// 3. Drop track,
/// 4. Do `media_manager.get_stream(caps)`
/// 5. Got new track and `getUserMedia` requests count = 2.
#[wasm_bindgen_test]
async fn new_track_if_previous_dropped() {
    let mock_navigator = MockNavigator::new();

    let media_manager = MediaManager::default();
    let constraints = {
        let mut constraints = MediaStreamSettings::new();
        constraints.audio(AudioTrackConstraints::new());
        constraints
    };

    // first request
    let mut tracks =
        media_manager.get_tracks(constraints.clone()).await.unwrap();

    assert_eq!(tracks.len(), 1);
    let (track1, track1_is_new) = tracks.pop().unwrap();

    assert_eq!(track1.kind(), MediaKind::Audio);
    assert!(track1_is_new);
    assert_eq!(mock_navigator.get_user_media_requests_count(), 1);

    // now drop track, and we got new track and second getUserMedia request
    let track1_id = track1.id();
    drop(track1);
    let mut tracks = media_manager.get_tracks(constraints).await.unwrap();

    assert_eq!(tracks.len(), 1);
    let (track2, track2_is_new) = tracks.pop().unwrap();

    assert!(track2_is_new);
    assert_ne!(track2.id(), track1_id);
    assert_eq!(track2.kind(), MediaKind::Audio);
    assert_eq!(mock_navigator.get_user_media_requests_count(), 2);

    mock_navigator.stop();
}

/// 1. Do `media_manager.get_stream({audio:true, video:true}})`;
/// 2. Do `media_manager.get_stream({audio:true}})`;
/// 3. Do `media_manager.get_stream({video:true}})`;
/// 4. Assert that same tracks were returned and no additional `getUserMedia`
///    request were made.
#[wasm_bindgen_test]
async fn request_audio_video_then_audio_then_video() {
    let mock_navigator = MockNavigator::new();

    let media_manager = MediaManager::default();
    let constraints = {
        let mut constraints = MediaStreamSettings::new();
        constraints.audio(AudioTrackConstraints::new());
        constraints.device_video(DeviceVideoTrackConstraints::new());
        constraints
    };

    let tracks = media_manager.get_tracks(constraints).await.unwrap();
    let (mut audio_tracks, mut video_tracks): (Vec<_>, Vec<_>) = tracks
        .into_iter()
        .partition(|(track, _)| track.kind() == MediaKind::Audio);
    assert_eq!(audio_tracks.len(), 1);
    assert_eq!(video_tracks.len(), 1);

    let (audio_track, audio_is_new) = audio_tracks.pop().unwrap();
    let (video_track, video_is_new) = video_tracks.pop().unwrap();
    assert!(audio_is_new);
    assert!(video_is_new);

    // request audio only
    let audio_constraints = {
        let mut constraints = MediaStreamSettings::new();
        constraints.audio(AudioTrackConstraints::new());
        constraints
    };
    let mut tracks = media_manager.get_tracks(audio_constraints).await.unwrap();
    assert_eq!(tracks.len(), 1);
    let (audio_track2, audio_track2_is_new) = tracks.pop().unwrap();
    assert!(!audio_track2_is_new);

    // request video only
    let video_constraints = {
        let mut constraints = MediaStreamSettings::new();
        constraints.device_video(DeviceVideoTrackConstraints::new());
        constraints
    };
    let mut tracks = media_manager.get_tracks(video_constraints).await.unwrap();
    assert_eq!(tracks.len(), 1);
    let (video_track2, video_track2_is_new) = tracks.pop().unwrap();
    assert!(!video_track2_is_new);

    assert_eq!(audio_track.id(), audio_track2.id());
    assert_eq!(video_track.id(), video_track2.id());
    assert_eq!(mock_navigator.get_user_media_requests_count(), 1);
    mock_navigator.stop();
}

/// 1. Do `media_manager.get_stream({audio:true, video:display}})`;
/// 2. Do `media_manager.get_stream({video:display}})`;
/// 3. Assert that same track is removed and only one getDisplayMedia request
///    were made.
#[wasm_bindgen_test]
async fn display_track_is_cached() {
    if is_firefox() {
        // getDisplayMedia is not mockable in ff atm
        return;
    }
    let mock_navigator = MockNavigator::new();

    let media_manager = MediaManager::default();
    let constraints = {
        let mut constraints = MediaStreamSettings::new();
        constraints.audio(AudioTrackConstraints::new());
        constraints.display_video(DisplayVideoTrackConstraints::new());
        constraints
    };

    let tracks = media_manager.get_tracks(constraints).await.unwrap();

    assert_eq!(tracks.len(), 2);

    let (video_track, video_track_is_new) = tracks
        .into_iter()
        .find(|(track, _)| track.kind() == MediaKind::Video)
        .unwrap();
    assert!(video_track_is_new);

    // do second request
    let constraints = {
        let mut constraints = MediaStreamSettings::new();
        constraints.display_video(DisplayVideoTrackConstraints::new());
        constraints
    };

    let mut tracks = media_manager.get_tracks(constraints).await.unwrap();

    assert_eq!(tracks.len(), 1);

    let (video_track2, video_track2_is_new) = tracks.pop().unwrap();
    assert!(!video_track2_is_new);
    assert_eq!(video_track.id(), video_track2.id());

    assert_eq!(mock_navigator.get_display_media_requests_count(), 1);
    assert_eq!(mock_navigator.get_user_media_requests_count(), 1);
    mock_navigator.stop();
}

/// Check that error is thrown if stream obtained via gUM request contains ended
/// track.
#[wasm_bindgen_test]
async fn new_tracks_should_be_live() {
    let media_manager = MediaManager::default();
    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());

    let track = Clone::clone(
        media_manager
            .get_tracks(constraints.clone())
            .await
            .unwrap()
            .pop()
            .unwrap()
            .0
            .sys_track(),
    );
    let ended_track = track.clone();
    ended_track.stop();

    let mock_navigator = MockNavigator::new();
    let return_stream =
        sys::MediaStream::new_with_tracks(&JsArray::from_iter(vec![
            Clone::clone(&track),
            ended_track,
        ]))
        .unwrap();
    mock_navigator.setUserMediaReturns(return_stream);

    if let Err(err) = media_manager.get_tracks(constraints).await {
        let err = err.into_inner();
        assert!(matches!(
            err,
            MediaManagerError::LocalTrackIsEnded(MediaKind::Audio)
        ));
    } else {
        panic!("expected err");
    }
    // Second track was stopped.
    assert_eq!(track.ready_state(), sys::MediaStreamTrackState::Ended);

    mock_navigator.stop();
}
