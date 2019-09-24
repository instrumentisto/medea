#![cfg(target_arch = "wasm32")]

use futures::prelude::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{MediaDeviceInfo, MediaDeviceKind, MediaStreamTrack};

use medea_jason::{
    media::{
        AudioTrackConstraints, MediaManager, MediaStreamConstraints,
        VideoTrackConstraints,
    },
    utils::window,
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
        .and_then(|infos| {
            Ok(js_sys::Array::from(&infos)
                .values()
                .into_iter()
                .map(|info| MediaDeviceInfo::from(info.unwrap()))
                .find(|device| {
                    device.kind() == MediaDeviceKind::Videoinput
                        && device.device_id() != "default"
                }))
        })
        .map(|device: Option<MediaDeviceInfo>| device.unwrap())
        .and_then(move |device| {
            let mut constraints = MediaStreamConstraints::new();
            let mut track_constraints = VideoTrackConstraints::new();
            track_constraints.device_id(device.device_id());
            constraints.video_mut().replace(track_constraints);

            MediaManager::default()
                .get_stream_by_constraints(constraints.clone())
                .map(move |stream| {
                    let track =
                        MediaStreamTrack::from(stream.get_tracks().pop());

                    assert!(constraints
                        .video()
                        .as_ref()
                        .unwrap()
                        .satisfies(&track));

                    ()
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
        .and_then(|infos| {
            Ok(js_sys::Array::from(&infos)
                .values()
                .into_iter()
                .map(|info| MediaDeviceInfo::from(info.unwrap()))
                .find(|device| {
                    device.kind() == MediaDeviceKind::Audioinput
                        && device.device_id() != "default"
                }))
        })
        .map(|device: Option<MediaDeviceInfo>| device.unwrap())
        .and_then(move |device| {
            let mut constraints = MediaStreamConstraints::new();
            let mut track_constraints = AudioTrackConstraints::new();
            track_constraints.device_id(device.device_id());
            constraints.audio_mut().replace(track_constraints);

            MediaManager::default()
                .get_stream_by_constraints(constraints.clone())
                .map(move |stream| {
                    let track =
                        MediaStreamTrack::from(stream.get_tracks().pop());

                    assert!(constraints
                        .audio()
                        .as_ref()
                        .unwrap()
                        .satisfies(&track));

                    ()
                })
                .map_err(|err| err.into())
        })
}
