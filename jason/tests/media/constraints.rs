#![cfg(target_arch = "wasm32")]

use futures::prelude::*;
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
    utils::{window, WasmErr},
};

wasm_bindgen_test_configure!(run_in_browser);

// 1. Get device id of non default video device from enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream by constraints;
// 4. Assert constraints.satisfies(stream.track());
#[wasm_bindgen_test(async)]
fn video_constraints_satisfies() -> impl Future<Item = (), Error = JsValue> {
    window()
        .navigator()
        .media_devices()
        .into_future()
        .and_then(|devices| devices.enumerate_devices())
        .and_then(JsFuture::from)
        .and_then(|infos| Ok(video_devices(&infos)))
        .map(|mut devices| devices.next().unwrap())
        .and_then(|device: MediaDeviceInfo| {
            let mut constraints = MediaStreamConstraints::new();
            let mut track_constraints = VideoTrackConstraints::new();
            track_constraints.device_id(device.device_id());
            constraints.video(track_constraints.clone());

            MediaManager::default()
                .get_stream_by_constraints(constraints.clone())
                .map(move |stream| {
                    let track =
                        MediaStreamTrack::from(stream.get_tracks().pop());

                    assert!(track_constraints.satisfies(&track));
                })
                .map_err(|err| err.into())
        })
}

// 1. Get device id of non default audio device from enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream by constraints;
// 4. Assert constraints.satisfies(stream.track());
#[wasm_bindgen_test(async)]
fn audio_constraints_satisfies() -> impl Future<Item = (), Error = JsValue> {
    window()
        .navigator()
        .media_devices()
        .into_future()
        .and_then(|devices| devices.enumerate_devices())
        .and_then(JsFuture::from)
        .and_then(|infos| Ok(audio_devices(&infos)))
        .map(|mut devices| devices.next().unwrap())
        .and_then(move |device: MediaDeviceInfo| {
            let mut constraints = MediaStreamConstraints::new();
            let mut track_constraints = AudioTrackConstraints::new();
            track_constraints.device_id(device.device_id());
            constraints.audio(track_constraints.clone());

            MediaManager::default()
                .get_stream_by_constraints(constraints.clone())
                .map(move |stream| {
                    let track =
                        MediaStreamTrack::from(stream.get_tracks().pop());

                    assert!(track_constraints.satisfies(&track));
                })
                .map_err(|err| err.into())
        })
}

/// Returns an iterator for non default audio input devices.
fn audio_devices(infos: &JsValue) -> impl Iterator<Item = MediaDeviceInfo> {
    js_sys::Array::from(&infos)
        .values()
        .into_iter()
        .map(|info| MediaDeviceInfo::from(info.unwrap()))
        .filter(|device| {
            device.kind() == MediaDeviceKind::Audioinput
                && device.device_id() != "default"
        })
}

/// Returns an iterator for non default video input devices.
fn video_devices(infos: &JsValue) -> impl Iterator<Item = MediaDeviceInfo> {
    js_sys::Array::from(&infos)
        .values()
        .into_iter()
        .map(|info| MediaDeviceInfo::from(info.unwrap()))
        .filter(|device| {
            device.kind() == MediaDeviceKind::Videoinput
                && device.device_id() != "default"
        })
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

// 1. Get device id of non default audio and video device from
// enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream by constraints;
// 4. Get stream again by this constraints;
// 4. Assert_eq!(stream1.get_stream_tracks(), stream2.get_stream_tracks());
#[wasm_bindgen_test(async)]
fn get_some_stream() -> impl Future<Item = (), Error = JsValue> {
    window()
        .navigator()
        .media_devices()
        .into_future()
        .and_then(|devices| devices.enumerate_devices())
        .and_then(JsFuture::from)
        .and_then(|infos| Ok((audio_devices(&infos), video_devices(&infos))))
        .and_then(move |(mut audio_devices, mut video_devices)| {
            let constraints =
                build_constraints(audio_devices.next(), video_devices.next());

            let manager = MediaManager::default();
            manager
                .get_stream_by_constraints(constraints.clone())
                .map(get_stream_tracks)
                .and_then(move |stream_tracks| {
                    manager
                        .get_stream_by_constraints(constraints.clone())
                        .map(get_stream_tracks)
                        .and_then(move |some_stream_tracks| {
                            let audio_track = stream_tracks
                                .iter()
                                .find(|track| track.kind() == "audio")
                                .unwrap();
                            let some_audio_track = some_stream_tracks
                                .iter()
                                .find(|track| track.kind() == "audio")
                                .unwrap();
                            assert_eq!(audio_track.id(), some_audio_track.id());

                            let video_track = stream_tracks
                                .iter()
                                .find(|track| track.kind() == "video")
                                .unwrap();
                            let some_video_track = some_stream_tracks
                                .iter()
                                .find(|track| track.kind() == "video")
                                .unwrap();
                            assert_eq!(video_track.id(), some_video_track.id());
                            Ok(())
                        })
                })
                .map_err(|err| err.into())
        })
}

// 1. Get device id of non default audio device from enumerate_devices();
// 2. Add it to constraints;
// 3. Get stream1 by constraints;
// 4. Get next device id of non default audio device from enumerate_devices();
// 2. Create new constraints;
// 3. Get stream2 by constraints;
// 4. Assert (stream1.track().id() != stream2.track().id());
#[wasm_bindgen_test(async)]
fn get_another_track_for_other_stream(
) -> impl Future<Item = (), Error = JsValue> {
    window()
        .navigator()
        .media_devices()
        .into_future()
        .and_then(|devices| devices.enumerate_devices())
        .and_then(JsFuture::from)
        .and_then(|infos| Ok(audio_devices(&infos)))
        .and_then(move |mut audio_devices| {
            let constraints = build_constraints(audio_devices.next(), None);

            let manager = MediaManager::default();
            manager
                .get_stream_by_constraints(constraints)
                .map(get_stream_tracks)
                .and_then(move |stream_tracks| {
                    let constraints =
                        build_constraints(audio_devices.next(), None);

                    manager
                        .get_stream_by_constraints(constraints)
                        .map(get_stream_tracks)
                        .and_then(move |another_stream_tracks| {
                            let audio_track = stream_tracks
                                .iter()
                                .find(|track| track.kind() == "audio")
                                .unwrap();
                            let another_audio_track = another_stream_tracks
                                .iter()
                                .find(|track| track.kind() == "audio")
                                .unwrap();
                            assert_ne!(
                                audio_track.id(),
                                another_audio_track.id()
                            );
                            Ok(())
                        })
                })
                .map_err(|err| err.into())
        })
}

#[wasm_bindgen_test(async)]
fn list_media_devices() -> impl Future<Item = (), Error = JsValue> {
    window()
        .navigator()
        .media_devices()
        .into_future()
        .and_then(|devices| devices.enumerate_devices())
        .and_then(JsFuture::from)
        .and_then(|infos| Ok((audio_devices(&infos), video_devices(&infos))))
        .and_then(move |(audio_devices, video_devices)| {
            for audio in audio_devices {
                WasmErr::from(format!(
                    "{:?}: {} - {}",
                    audio.kind(),
                    audio.label(),
                    audio.device_id()
                ))
                .log_err()
            }
            for video in video_devices {
                WasmErr::from(format!(
                    "{:?}: {} - {}",
                    video.kind(),
                    video.label(),
                    video.device_id()
                ))
                .log_err()
            }
            Ok(())
        })
}
