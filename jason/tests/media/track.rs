#![cfg(target_arch = "wasm32")]

use futures::channel::oneshot;
use medea_jason::{
    media::MediaManager, DeviceVideoTrackConstraints, MediaTracksSettings,
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen_test::*;
use web_sys::MediaStreamTrackState;

use crate::{get_audio_track, timeout};

/// Assert that track is stopped when all strong refs are dropped.
#[wasm_bindgen_test]
async fn track_autostop() {
    let media_manager = MediaManager::default();
    let mut caps = MediaTracksSettings::new();
    caps.device_video(DeviceVideoTrackConstraints::new());

    let (stream, is_new) = media_manager.get_tracks(caps).await.unwrap();
    assert!(is_new);

    let mut tracks = stream;
    assert_eq!(1, tracks.len());
    let strong_track = tracks.pop().unwrap();
    let sys_track = Clone::clone(strong_track.as_ref());
    let weak_track = strong_track.downgrade();

    assert!(sys_track.ready_state() == MediaStreamTrackState::Live);
    drop(strong_track);
    assert!(sys_track.ready_state() == MediaStreamTrackState::Ended);
    assert!(!weak_track.can_be_upgraded());
}

#[wasm_bindgen_test]
async fn on_track_enabled_works() {
    let track = get_audio_track().await;

    let (test_tx, test_rx) = oneshot::channel();
    track.on_enabled(
        Closure::once_into_js(move || {
            test_tx.send(()).unwrap();
        })
        .into(),
    );

    track.set_enabled(false);
    track.set_enabled(true);

    timeout(100, test_rx).await.unwrap().unwrap();
}

#[wasm_bindgen_test]
async fn on_track_disabled_works() {
    let track = get_audio_track().await;

    let (test_tx, test_rx) = oneshot::channel();
    track.on_disabled(
        Closure::once_into_js(move || {
            test_tx.send(()).unwrap();
        })
        .into(),
    );

    track.set_enabled(false);

    timeout(100, test_rx).await.unwrap().unwrap();
}
