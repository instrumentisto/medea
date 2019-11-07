#![cfg(target_arch = "wasm32")]

use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{
    MediaDeviceInfo, MediaDeviceKind, MediaStream, MediaStreamTrack,
};

use medea_jason::{
    media::{
        AudioTrackConstraints, MediaManager, MediaStreamConstraints,
        VideoTrackConstraints,
    },
    utils::window,
};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn not_satisfies_stopped_video_track() {
    let video_device = video_devices().await.unwrap().pop().unwrap();

    let mut constraints = MediaStreamConstraints::new();
    let mut track_constraints = VideoTrackConstraints::new();
    track_constraints.device_id(video_device.device_id());
    constraints.video(track_constraints.clone());

    let media_manager = MediaManager::default();
    let (stream, _) =
        media_manager.get_stream(constraints.clone()).await.unwrap();

    assert!(stream.get_tracks().length() == 1);

    let track = MediaStreamTrack::from(stream.get_tracks().pop());
    track.stop();

    assert!(track.kind() == "video");
    assert!(!track_constraints.satisfies(&track));
}

#[wasm_bindgen_test]
async fn not_satisfies_stopped_audio_track() {
    let audio_device = audio_devices().await.unwrap().pop().unwrap();

    let mut constraints = MediaStreamConstraints::new();
    let mut track_constraints = AudioTrackConstraints::new();
    track_constraints.device_id(audio_device.device_id());
    constraints.audio(track_constraints.clone());

    let media_manager = MediaManager::default();
    let (stream, _) =
        media_manager.get_stream(constraints.clone()).await.unwrap();

    assert!(stream.get_tracks().length() == 1);

    let track = MediaStreamTrack::from(stream.get_tracks().pop());
    track.stop();

    assert!(track.kind() == "audio");
    assert!(!track_constraints.satisfies(&track));
}

// 1. Get device id of non default video device from enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream by constraints;
// 4. Assert constraints.satisfies(stream.track());
#[wasm_bindgen_test]
async fn video_constraints_satisfies() {
    let video_device = video_devices().await.unwrap().pop().unwrap();

    let mut constraints = MediaStreamConstraints::new();
    let mut track_constraints = VideoTrackConstraints::new();
    track_constraints.device_id(video_device.device_id());
    constraints.video(track_constraints.clone());

    let media_manager = MediaManager::default();
    let (stream, _) =
        media_manager.get_stream(constraints.clone()).await.unwrap();

    assert!(stream.get_tracks().length() == 1);

    let track = MediaStreamTrack::from(stream.get_tracks().pop());

    assert!(track.kind() == "video");
    assert!(track_constraints.satisfies(&track));
}

// 1. Get device id of non default audio device from enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream by constraints;
// 4. Assert constraints.satisfies(stream.track());
#[wasm_bindgen_test]
async fn audio_constraints_satisfies() {
    let audio_device = audio_devices().await.unwrap().pop().unwrap();

    let mut constraints = MediaStreamConstraints::new();
    let mut track_constraints = AudioTrackConstraints::new();
    track_constraints.device_id(audio_device.device_id());
    constraints.audio(track_constraints.clone());

    let media_manager = MediaManager::default();
    let (stream, _) =
        media_manager.get_stream(constraints.clone()).await.unwrap();

    assert!(stream.get_tracks().length() == 1);

    let track = MediaStreamTrack::from(stream.get_tracks().pop());

    assert!(track.kind() == "audio");
    assert!(track_constraints.satisfies(&track));
}

// 1. Get device id of non default video device from enumerate_devices();
// 2. Get device id of non default audio device from enumerate_devices();
// 2. Add both to constraints;
// 3. Get stream by constraints;
// 4. Assert that we got stream with 2 tracks;
// 5. Assert audio_constraint.satisfies(stream.audio_track());
// 6. Assert video_constraint.satisfies(stream.video_track()).
#[wasm_bindgen_test]
async fn both_constraints_satisfies() {
    let audio_device = audio_devices().await.unwrap().pop().unwrap();
    let video_device = video_devices().await.unwrap().pop().unwrap();

    let constraints = {
        let mut constraints = MediaStreamConstraints::new();

        let mut audio_constraints = AudioTrackConstraints::new();
        audio_constraints.device_id(audio_device.device_id());

        let mut video_constraints = VideoTrackConstraints::new();
        video_constraints.device_id(video_device.device_id());

        constraints.audio(audio_constraints);
        constraints.video(video_constraints);

        constraints
    };
    let media_manager = MediaManager::default();

    let (stream, _) =
        media_manager.get_stream(constraints.clone()).await.unwrap();

    assert!(stream.get_tracks().length() == 2);

    let video_constraints = constraints.get_video().clone().unwrap();
    let audio_constraints = constraints.get_audio().clone().unwrap();

    let audio_track = MediaStreamTrack::from(stream.get_audio_tracks().pop());
    let video_track = MediaStreamTrack::from(stream.get_video_tracks().pop());

    assert!(audio_track.kind() == "audio");
    assert!(audio_constraints.satisfies(&audio_track));

    assert!(video_track.kind() == "video");
    assert!(video_constraints.satisfies(&video_track));
}

// 1. Get device id of non default audio and video device from
// enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream by constraints;
// 4. Get stream again by this constraints;
// 5. Assert_eq!(stream1.get_stream_tracks(), stream2.get_stream_tracks());
#[wasm_bindgen_test]
async fn equal_constraints_produce_equal_streams() {
    let audio_devices = audio_devices().await.unwrap();
    let video_devices = video_devices().await.unwrap();

    let constraints = build_constraints(
        audio_devices.into_iter().next(),
        video_devices.into_iter().next(),
    );

    let manager = MediaManager::default();

    let (stream, _) = manager.get_stream(constraints.clone()).await.unwrap();
    let stream_tracks = get_stream_tracks(stream);

    let (stream, _) = manager.get_stream(constraints.clone()).await.unwrap();
    let another_stream_tracks = get_stream_tracks(stream);

    let audio_track = stream_tracks
        .iter()
        .find(|track| track.kind() == "audio")
        .unwrap();
    let some_audio_track = another_stream_tracks
        .iter()
        .find(|track| track.kind() == "audio")
        .unwrap();
    assert_eq!(audio_track.id(), some_audio_track.id());

    let video_track = stream_tracks
        .iter()
        .find(|track| track.kind() == "video")
        .unwrap();
    let some_video_track = another_stream_tracks
        .iter()
        .find(|track| track.kind() == "video")
        .unwrap();
    assert_eq!(video_track.id(), some_video_track.id());
}

// 0. If audio_devices.len() > 1 (otherwise this test makes no sense);
// 1. Get device id of non default audio device from enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream1 by constraints;
// 4. Get next device id of non default audio device from enumerate_devices();
// 5. Create new constraints;
// 6. Get stream2 by constraints;
// 7. Assert (stream1.track().id() != stream2.track().id());
#[wasm_bindgen_test]
async fn different_constraints_produce_different_streams() {
    let mut audio_devices = audio_devices().await.unwrap().into_iter();

    if audio_devices.len() > 1 {
        let constraints = build_constraints(audio_devices.next(), None);

        let manager = MediaManager::default();

        let (stream, _) = manager.get_stream(constraints).await.unwrap();
        let stream_tracks = get_stream_tracks(stream);

        let constraints = build_constraints(audio_devices.next(), None);
        let (another_stream, _) =
            manager.get_stream(constraints).await.unwrap();
        let another_stream_tracks = get_stream_tracks(another_stream);

        let audio_track = stream_tracks
            .iter()
            .find(|track| track.kind() == "audio")
            .unwrap();
        let another_audio_track = another_stream_tracks
            .iter()
            .find(|track| track.kind() == "audio")
            .unwrap();
        assert_ne!(audio_track.id(), another_audio_track.id());
    }
}

/// Returns all registered media devices.
async fn get_media_devices() -> Result<Vec<MediaDeviceInfo>, JsValue> {
    let media_devices = window().navigator().media_devices()?;
    let media_devices =
        JsFuture::from(media_devices.enumerate_devices()?).await?;

    Ok(js_sys::Array::from(&media_devices)
        .values()
        .into_iter()
        .map(|item| MediaDeviceInfo::from(item.unwrap()))
        .collect())
}

/// Returns an iterator for non default audio input devices.
async fn audio_devices() -> Result<Vec<MediaDeviceInfo>, JsValue> {
    let devices = get_media_devices().await?;

    Ok(devices
        .into_iter()
        .filter(|device| {
            device.kind() == MediaDeviceKind::Audioinput
                && device.device_id() != "default"
        })
        .collect())
}

/// Returns an iterator for non default video input devices.
async fn video_devices() -> Result<Vec<MediaDeviceInfo>, JsValue> {
    let devices = get_media_devices().await?;

    Ok(devices
        .into_iter()
        .filter(|device| {
            device.kind() == MediaDeviceKind::Videoinput
                && device.device_id() != "default"
        })
        .collect())
}

/// Build [MediaStreamConstraints] for given device.
fn build_constraints(
    audio_device: Option<MediaDeviceInfo>,
    video_device: Option<MediaDeviceInfo>,
) -> MediaStreamConstraints {
    let mut constraints = MediaStreamConstraints::new();
    if let Some(audio) = audio_device {
        let mut track_constraints = AudioTrackConstraints::new();
        track_constraints.device_id(audio.device_id());
        constraints.audio(track_constraints);
    }
    if let Some(video) = video_device {
        let mut track_constraints = VideoTrackConstraints::new();
        track_constraints.device_id(video.device_id());
        constraints.video(track_constraints);
    }
    constraints
}

/// Returns [MediaStreamTrack]s of given [MediaStream].
fn get_stream_tracks(stream: MediaStream) -> Vec<MediaStreamTrack> {
    js_sys::try_iter(&stream.get_tracks())
        .unwrap()
        .unwrap()
        .map(|tr| web_sys::MediaStreamTrack::from(tr.unwrap()))
        .collect()
}
