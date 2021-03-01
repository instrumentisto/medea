#![cfg(target_arch = "wasm32")]

use medea_client_api_proto::{MediaSourceKind, VideoSettings};
use medea_jason::{
    media::{
        AudioTrackConstraints, DeviceVideoTrackConstraints,
        DisplayVideoTrackConstraints, MediaKind, MediaManager,
        MediaStreamSettings, MultiSourceTracksConstraints, VideoSource,
    },
    platform::get_property_by_name,
};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{MediaDeviceInfo, MediaDeviceKind};

use crate::{is_firefox, window};

wasm_bindgen_test_configure!(run_in_browser);

// 1. Get device id of non default video device from enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream by constraints;
// 4. Assert constraints.satisfies(stream.track());
#[wasm_bindgen_test]
async fn video_constraints_satisfies() {
    let video_device = video_devices().await.unwrap().pop().unwrap();

    let mut constraints = MediaStreamSettings::new();
    let mut track_constraints = DeviceVideoTrackConstraints::new();
    track_constraints.device_id(video_device.device_id());
    constraints.device_video(track_constraints.clone());

    let media_manager = MediaManager::default();
    let mut tracks =
        media_manager.get_tracks(constraints.clone()).await.unwrap();

    assert_eq!(tracks.len(), 1);

    let track = tracks.pop().unwrap().0;

    assert_eq!(track.kind(), MediaKind::Video);
    assert!(track_constraints.satisfies(track.as_ref()));
}

// 1. Get device id of non default audio device from enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream by constraints;
// 4. Assert constraints.satisfies(stream.track());
#[wasm_bindgen_test]
async fn audio_constraints_satisfies() {
    let audio_device = audio_devices().await.unwrap().pop().unwrap();

    let mut constraints = MediaStreamSettings::new();
    let mut track_constraints = AudioTrackConstraints::new();
    track_constraints.device_id(audio_device.device_id());
    constraints.audio(track_constraints.clone());

    let media_manager = MediaManager::default();
    let mut tracks =
        media_manager.get_tracks(constraints.clone()).await.unwrap();

    assert_eq!(tracks.len(), 1);

    let track = tracks.pop().unwrap().0;

    assert_eq!(track.kind(), MediaKind::Audio);
    assert!(track_constraints.satisfies(track.as_ref()));
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
        let mut constraints = MediaStreamSettings::new();

        let mut audio_constraints = AudioTrackConstraints::new();
        audio_constraints.device_id(audio_device.device_id());

        let mut video_constraints = DeviceVideoTrackConstraints::new();
        video_constraints.device_id(video_device.device_id());

        constraints.audio(audio_constraints);
        constraints.device_video(video_constraints);

        constraints
    };
    let media_manager = MediaManager::default();

    let tracks = media_manager.get_tracks(constraints.clone()).await.unwrap();

    let video_constraints = constraints.get_device_video().clone().unwrap();
    let audio_constraints = constraints.get_audio().clone();

    assert_eq!(tracks.len(), 2);

    let (mut audio, mut video): (Vec<_>, Vec<_>) = tracks
        .into_iter()
        .partition(|(track, _)| match track.kind() {
            MediaKind::Audio => true,
            MediaKind::Video => false,
        });

    let audio_track = audio.pop().unwrap().0;
    let video_track = video.pop().unwrap().0;

    assert_eq!(audio_track.kind(), MediaKind::Audio);
    assert!(audio_constraints.satisfies(audio_track.as_ref()));

    assert_eq!(video_track.kind(), MediaKind::Video);
    assert!(video_constraints.satisfies(video_track.as_ref()));
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

    let tracks = manager.get_tracks(constraints.clone()).await.unwrap();
    let another_tracks = manager.get_tracks(constraints.clone()).await.unwrap();

    let audio_track = &tracks
        .iter()
        .find(|(track, _)| track.kind() == MediaKind::Audio)
        .unwrap()
        .0;
    let some_audio_track = &another_tracks
        .iter()
        .find(|(track, _)| track.kind() == MediaKind::Audio)
        .unwrap()
        .0;
    assert_eq!(audio_track.id(), some_audio_track.id());

    let video_track = &tracks
        .iter()
        .find(|(track, _)| track.kind() == MediaKind::Video)
        .unwrap()
        .0;
    let some_video_track = &another_tracks
        .iter()
        .find(|(track, _)| track.kind() == MediaKind::Video)
        .unwrap()
        .0;
    assert_eq!(video_track.id(), some_video_track.id());
}

/// 1. If audio_devices.len() > 1 (otherwise this test makes no sense);
/// 2. Get device id of non default audio device from enumerate_devices();
/// 3. Add it to constraints;
/// 4. Get stream1 by constraints;
/// 5. Get next device id of non default audio device from
/// `enumerate_devices()`; 6. Create new constraints;
/// 7. Get stream2 by constraints;
/// 8. Assert `(stream1.track().id() != stream2.track().id())`.
#[wasm_bindgen_test]
async fn different_constraints_produce_different_streams() {
    let mut audio_devices = audio_devices().await.unwrap().into_iter();

    if audio_devices.len() > 1 {
        let constraints = build_constraints(audio_devices.next(), None);

        let manager = MediaManager::default();

        let tracks = manager.get_tracks(constraints).await.unwrap();

        let constraints = build_constraints(audio_devices.next(), None);
        let another_tracks = manager.get_tracks(constraints).await.unwrap();

        let audio_track = &tracks
            .iter()
            .find(|(track, _)| track.kind() == MediaKind::Audio)
            .unwrap()
            .0;
        let another_audio_track = &another_tracks
            .iter()
            .find(|(track, _)| track.kind() == MediaKind::Audio)
            .unwrap()
            .0;
        assert_ne!(audio_track.id(), another_audio_track.id());
    }
}

// Make sure that MediaStreamConstraints{audio:false, video:false} =>
// Device({audio:false, video:false})
#[wasm_bindgen_test]
async fn multi_source_media_stream_constraints_build1() {
    let constraints: Option<MultiSourceTracksConstraints> =
        MediaStreamSettings::new().into();

    assert!(constraints.is_none());
}

// Make sure that MediaStreamConstraints{audio:true, video:false} =>
// Device({audio:true, video:false})
#[wasm_bindgen_test]
async fn multi_source_media_stream_constraints_build2() {
    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());

    let constraints: Option<MultiSourceTracksConstraints> = constraints.into();

    match constraints {
        Some(MultiSourceTracksConstraints::Device(constraints)) => {
            let has_video = get_property_by_name(
                constraints.as_ref(),
                "video",
                js_val_to_option,
            )
            .is_some();
            let has_audio = get_property_by_name(
                constraints.as_ref(),
                "audio",
                js_val_to_option,
            )
            .is_some();

            assert!(!has_video);
            assert!(has_audio);
        }
        _ => unreachable!(),
    };
}

// Make sure that MediaStreamConstraints{audio:true, video:device} =>
// Device({audio:true, video:true})
#[wasm_bindgen_test]
async fn multi_source_media_stream_constraints_build3() {
    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());
    constraints.device_video(DeviceVideoTrackConstraints::new());

    let constraints: Option<MultiSourceTracksConstraints> = constraints.into();

    match constraints {
        Some(MultiSourceTracksConstraints::Device(constraints)) => {
            let has_video = get_property_by_name(
                constraints.as_ref(),
                "video",
                js_val_to_option,
            )
            .is_some();
            let has_audio = get_property_by_name(
                constraints.as_ref(),
                "audio",
                js_val_to_option,
            )
            .is_some();

            assert!(has_video);
            assert!(has_audio);
        }
        _ => unreachable!(),
    };
}

// Make sure that MediaStreamConstraints{audio:true, video:display} =>
// DeviceAndDisplay({audio:true, video:false}, {audio:false, video:display})
#[wasm_bindgen_test]
async fn multi_source_media_stream_constraints_build4() {
    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());
    constraints.display_video(DisplayVideoTrackConstraints::new());

    let constraints: Option<MultiSourceTracksConstraints> = constraints.into();

    match constraints {
        Some(MultiSourceTracksConstraints::DeviceAndDisplay(
            device,
            display,
        )) => {
            let device_has_video = get_property_by_name(
                device.as_ref(),
                "video",
                js_val_to_option,
            )
            .is_some();
            let device_has_audio = get_property_by_name(
                &device.as_ref(),
                "audio",
                js_val_to_option,
            )
            .is_some();
            let display_has_video = get_property_by_name(
                display.as_ref(),
                "video",
                js_val_to_option,
            )
            .is_some();
            let display_has_audio = get_property_by_name(
                display.as_ref(),
                "audio",
                js_val_to_option,
            )
            .is_some();

            assert!(!device_has_video);
            assert!(device_has_audio);
            assert!(display_has_video);
            assert!(!display_has_audio);
        }
        _ => unreachable!(),
    };
}

// Make sure that MediaStreamConstraints{audio:false, video:device} =>
// Device({audio:false, video:true})
#[wasm_bindgen_test]
async fn multi_source_media_stream_constraints_build5() {
    let mut constraints = MediaStreamSettings::new();
    constraints.device_video(DeviceVideoTrackConstraints::new());

    let constraints: Option<MultiSourceTracksConstraints> = constraints.into();

    match constraints {
        Some(MultiSourceTracksConstraints::Device(constraints)) => {
            let has_video = get_property_by_name(
                constraints.as_ref(),
                "video",
                js_val_to_option,
            )
            .is_some();
            let has_audio = get_property_by_name(
                constraints.as_ref(),
                "audio",
                js_val_to_option,
            )
            .is_some();

            assert!(has_video);
            assert!(!has_audio);
        }
        _ => unreachable!(),
    };
}

// Make sure that MediaStreamConstraints{audio:false, video:display} =>
// Display({audio:false, video:true})
#[wasm_bindgen_test]
async fn multi_source_media_stream_constraints_build6() {
    let mut constraints = MediaStreamSettings::new();
    constraints.display_video(DisplayVideoTrackConstraints::new());

    let constraints: Option<MultiSourceTracksConstraints> = constraints.into();

    match constraints {
        Some(MultiSourceTracksConstraints::Display(constraints)) => {
            let has_video = get_property_by_name(
                constraints.as_ref(),
                "video",
                js_val_to_option,
            )
            .is_some();
            let has_audio = get_property_by_name(
                constraints.as_ref(),
                "audio",
                js_val_to_option,
            )
            .is_some();

            assert!(has_video);
            assert!(!has_audio);
        }
        _ => unreachable!(),
    };
}

fn get_device_video_track_constraints() -> DeviceVideoTrackConstraints {
    match VideoSource::from(VideoSettings {
        required: true,
        source_kind: MediaSourceKind::Device,
    }) {
        VideoSource::Device(device) => device,
        _ => unreachable!(),
    }
}

// Make sure that MediaStreamConstraints{audio:true, video:any} =>
// Device({audio:true, video:true})
#[wasm_bindgen_test]
async fn multi_source_media_stream_constraints_build7() {
    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());
    constraints.device_video(get_device_video_track_constraints());

    let constraints: Option<MultiSourceTracksConstraints> = constraints.into();

    match constraints {
        Some(MultiSourceTracksConstraints::Device(constraints)) => {
            let has_video = get_property_by_name(
                constraints.as_ref(),
                "video",
                js_val_to_option,
            )
            .is_some();
            let has_audio = get_property_by_name(
                constraints.as_ref(),
                "audio",
                js_val_to_option,
            )
            .is_some();

            assert!(has_video);
            assert!(has_audio);
        }
        _ => unreachable!(),
    };
}

// Make sure that MediaStreamConstraints{audio:false, video:any} =>
// Device({audio:false, video:true})
#[wasm_bindgen_test]
async fn multi_source_media_stream_constraints_build8() {
    let mut constraints = MediaStreamSettings::new();
    constraints.device_video(get_device_video_track_constraints());

    let constraints: Option<MultiSourceTracksConstraints> = constraints.into();

    match constraints {
        Some(MultiSourceTracksConstraints::Device(constraints)) => {
            let has_video = get_property_by_name(
                constraints.as_ref(),
                "video",
                js_val_to_option,
            )
            .is_some();
            let has_audio = get_property_by_name(
                constraints.as_ref(),
                "audio",
                js_val_to_option,
            )
            .is_some();

            assert!(has_video);
            assert!(!has_audio);
        }
        _ => unreachable!(),
    };
}

#[wasm_bindgen_test]
async fn merge_audio() {}

#[wasm_bindgen_test]
async fn merge_video() {}

// Maps undefined to None.
fn js_val_to_option(val: JsValue) -> Option<JsValue> {
    if val.is_undefined() {
        None
    } else {
        Some(val)
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
) -> MediaStreamSettings {
    let mut constraints = MediaStreamSettings::new();
    if let Some(audio) = audio_device {
        let mut track_constraints = AudioTrackConstraints::new();
        track_constraints.device_id(audio.device_id());
        constraints.audio(track_constraints);
    }
    if let Some(video) = video_device {
        let mut track_constraints = DeviceVideoTrackConstraints::new();
        track_constraints.device_id(video.device_id());
        constraints.device_video(track_constraints);
    }
    constraints
}

/// Tests that simultaneous device and display constraining works.
#[wasm_bindgen_test]
async fn simultaneous_device_and_display() {
    if is_firefox() {
        return;
    }
    let audio_device = audio_devices().await.unwrap().pop().unwrap();
    let video_device = video_devices().await.unwrap().pop().unwrap();

    let constraints = {
        let mut constraints = MediaStreamSettings::new();

        let mut audio_constraints = AudioTrackConstraints::new();
        audio_constraints.device_id(audio_device.device_id());

        let mut video_constraints = DeviceVideoTrackConstraints::new();
        video_constraints.device_id(video_device.device_id());

        constraints.audio(audio_constraints);
        constraints.device_video(video_constraints);
        constraints.display_video(DisplayVideoTrackConstraints::new());

        constraints
    };
    let media_manager = MediaManager::default();

    let tracks = media_manager.get_tracks(constraints.clone()).await.unwrap();

    let device_video_constraints =
        constraints.get_device_video().clone().unwrap();
    let display_video_constraints =
        constraints.get_display_video().clone().unwrap();
    let audio_constraints = constraints.get_audio().clone();

    assert_eq!(tracks.len(), 3);

    let (mut audio, mut video): (Vec<_>, Vec<_>) = tracks
        .into_iter()
        .partition(|(track, _)| match track.kind() {
            MediaKind::Audio => true,
            MediaKind::Video => false,
        });

    let audio_track = audio.pop().unwrap().0;
    assert_eq!(audio_track.kind(), MediaKind::Audio);
    assert!(audio_constraints.satisfies(audio_track.as_ref()));

    let display_video_track = video.pop().unwrap().0;
    assert_eq!(display_video_track.kind(), MediaKind::Video);
    assert!(display_video_constraints.satisfies(display_video_track.as_ref()));
    assert_eq!(
        display_video_track.media_source_kind(),
        MediaSourceKind::Display
    );

    let device_video_track = video.pop().unwrap().0;
    assert_eq!(device_video_track.kind(), MediaKind::Video);
    assert!(device_video_constraints.satisfies(device_video_track.as_ref()));
    assert_eq!(
        device_video_track.media_source_kind(),
        MediaSourceKind::Device
    );
}
