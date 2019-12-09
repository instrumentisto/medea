#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use wasm_bindgen_test::*;

use medea_jason::{
    api::{RoomStream, StreamSourceError},
    media::{AudioTrackConstraints, MediaManager, MediaStreamConstraints},
    peer::{MediaSource, MediaStreamHandle, StreamRequest},
    utils::JasonError,
};

use crate::{get_test_tracks, wait_and_check_test_result, MockNavigator};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn returns_stored_stream_if_it_satisfies_request() {
    let media_manager = Rc::new(MediaManager::default());
    let constraints = {
        let mut constraints = MediaStreamConstraints::new();
        let audio_constraints = AudioTrackConstraints::new();
        constraints.audio(audio_constraints);
        constraints
    };
    let (stream, _) = media_manager.get_stream(constraints).await.unwrap();
    let room_store = RoomStream::new(media_manager);
    room_store.store_local_stream(stream);
    let (audio, _) = get_test_tracks();
    let mut request = StreamRequest::default();
    request.add_track_request(audio.id, audio.media_type);
    let result = room_store.get_media_stream(request).await;
    match result {
        Ok(media_stream) => assert!(
            media_stream.has_track(audio.id),
            "Not found requested track"
        ),
        Err(_) => assert!(false, "Must be success"),
    }
}

#[wasm_bindgen_test]
async fn fired_on_success_callback_if_received_new_stream_from_media_manager() {
    let room_store = RoomStream::new(Rc::new(MediaManager::default()));
    let (cb, test_result) = js_callback!(|s: MediaStreamHandle| {
        cb_assert_eq!(s.get_media_stream().is_ok(), true);
    });
    room_store.on_success(cb.into());
    let (audio, video) = get_test_tracks();
    let mut request = StreamRequest::default();
    request.add_track_request(audio.id, audio.media_type);
    request.add_track_request(video.id, video.media_type);
    let result = room_store.get_media_stream(request).await;
    match result {
        Ok(media_stream) => {
            assert!(media_stream.has_track(audio.id), "Not found audio track");
            assert!(media_stream.has_track(video.id), "Not found video track");
        }
        Err(_) => assert!(false, "Must be success"),
    }
    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn fired_on_fail_if_media_manager_failed() {
    let mock_navigator = MockNavigator::new();
    mock_navigator.error_get_user_media("failed_get_user_media".into());
    let room_store = RoomStream::new(Rc::new(MediaManager::default()));
    let (cb, test_result) = js_callback!(|err: JasonError| {
        cb_assert_eq!(&err.name(), "CouldNotGetLocalMedia");
        cb_assert_eq!(
            &err.message(),
            "Failed to get local stream: MediaDevices.getUserMedia() failed: \
             Unknown JS error: failed_get_user_media"
        );
    });
    room_store.on_fail(cb.into());
    let (audio, _) = get_test_tracks();
    let mut request = StreamRequest::default();
    request.add_track_request(audio.id, audio.media_type);
    let result = room_store.get_media_stream(request).await;
    match result {
        Ok(_) => assert!(false, "Cannot be success"),
        Err(err) => match err.as_ref() {
            StreamSourceError::CouldNotGetLocalMedia(_) => assert!(true),
            _ => assert!(false, "Expected `CouldNotGetLocalMedia` error"),
        },
    }
    wait_and_check_test_result(test_result, move || mock_navigator.stop())
        .await;
}

#[wasm_bindgen_test]
async fn fired_on_fail_if_stored_stream_not_satisfied_request() {
    let media_manager = Rc::new(MediaManager::default());
    let constraints = {
        let mut constraints = MediaStreamConstraints::new();
        let audio_constraints = AudioTrackConstraints::new();
        constraints.audio(audio_constraints);
        constraints
    };
    let (stream, _) = media_manager.get_stream(constraints).await.unwrap();
    let room_store = RoomStream::new(media_manager);
    room_store.store_local_stream(stream);
    let (cb, test_result) = js_callback!(|err: JasonError| {
        cb_assert_eq!(&err.name(), "InvalidLocalStream");
        cb_assert_eq!(
            &err.message(),
            "Invalid local stream: provided MediaStream was expected to have \
             single video track"
        );
    });
    room_store.on_fail(cb.into());
    let (audio, video) = get_test_tracks();
    let mut request = StreamRequest::default();
    request.add_track_request(audio.id, audio.media_type);
    request.add_track_request(video.id, video.media_type);
    let result = room_store.get_media_stream(request).await;
    match result {
        Ok(_) => assert!(false, "Cannot be success"),
        Err(err) => match err.as_ref() {
            StreamSourceError::InvalidLocalStream(_) => assert!(true),
            _ => assert!(false, "Expected `InvalidLocalStream` error"),
        },
    }
    wait_and_check_test_result(test_result, || {}).await;
}
