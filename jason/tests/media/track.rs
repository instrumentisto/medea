#![cfg(target_arch = "wasm32")]

use std::rc::{Rc, Weak};

use futures::channel::oneshot;
use medea_jason::media::{
    track::remote, DeviceVideoTrackConstraints, MediaManager,
    MediaStreamSettings,
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen_test::*;

use crate::{get_audio_track, timeout};

/// Assert that track is stopped when all strong refs are dropped.
#[wasm_bindgen_test]
async fn track_autostop() {
    let media_manager = MediaManager::default();
    let mut caps = MediaStreamSettings::new();
    caps.device_video(DeviceVideoTrackConstraints::new());

    let mut tracks = media_manager.get_tracks(caps).await.unwrap();

    assert_eq!(1, tracks.len());
    let (strong_track, strong_track_is_new) = tracks.pop().unwrap();
    assert!(strong_track_is_new);
    let sys_track = Clone::clone(strong_track.as_ref().as_ref().as_ref());
    let weak_track = Rc::downgrade(&strong_track);

    assert!(sys_track.ready_state() == web_sys::MediaStreamTrackState::Live);
    drop(strong_track);
    assert!(sys_track.ready_state() == web_sys::MediaStreamTrackState::Ended);
    assert_eq!(Weak::strong_count(&weak_track), 0);
}

#[wasm_bindgen_test]
async fn on_track_enabled_works() {
    let api_track = get_audio_track().await;
    let core_track: remote::Track = api_track.clone().into();

    let core_track_clone = core_track.clone();
    let (test_tx, test_rx) = oneshot::channel();
    api_track.on_enabled(
        Closure::once_into_js(move || {
            assert!(core_track_clone.enabled());
            test_tx.send(()).unwrap();
        })
        .into(),
    );

    core_track.set_enabled(false);
    assert!(!api_track.enabled());
    core_track.set_enabled(true);
    assert!(api_track.enabled());

    timeout(100, test_rx).await.unwrap().unwrap();
}

#[wasm_bindgen_test]
async fn on_track_disabled_works() {
    let track = get_audio_track().await;

    let track_clone = track.clone();
    let (test_tx, test_rx) = oneshot::channel();
    track.on_disabled(
        Closure::once_into_js(move || {
            assert!(!track_clone.enabled());
            test_tx.send(()).unwrap();
        })
        .into(),
    );

    let track = remote::Track::from(track);
    track.set_enabled(false);

    timeout(100, test_rx).await.unwrap().unwrap();
}
