#![cfg(target_arch = "wasm32")]

use std::rc::{Rc, Weak};

use futures::{
    channel::{mpsc, oneshot},
    StreamExt as _,
};
use medea_jason::{
    media::MediaManager, DeviceVideoTrackConstraints, MediaStreamSettings,
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen_test::*;
use web_sys::MediaStreamTrackState;

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
    let sys_track = Clone::clone(strong_track.sys_track());
    let weak_track = Rc::downgrade(&strong_track);

    assert!(sys_track.ready_state() == MediaStreamTrackState::Live);
    drop(strong_track);
    assert!(sys_track.ready_state() == MediaStreamTrackState::Ended);
    assert_eq!(Weak::strong_count(&weak_track), 0);
}

#[wasm_bindgen_test]
async fn on_track_enabled_works() {
    let track = get_audio_track().await;

    let track_clone = track.clone();
    let (test_tx, test_rx) = oneshot::channel();
    track.on_enabled(
        Closure::once_into_js(move || {
            assert!(track_clone.js_enabled());
            test_tx.send(()).unwrap();
        })
        .into(),
    );

    let (dont_fire_tx, mut dont_fire_rx) = mpsc::unbounded();
    let dont_fire = || {
        let tx = dont_fire_tx.clone();
        Closure::once_into_js(move || {
            tx.unbounded_send(()).unwrap();
        })
        .into()
    };
    track.on_muted(dont_fire());
    track.on_unmuted(dont_fire());
    track.on_stopped(dont_fire());

    track.set_enabled(false);
    assert!(!track.muted());
    assert!(!track.js_enabled());
    track.set_enabled(true);
    assert!(!track.muted());
    assert!(track.js_enabled());

    timeout(100, test_rx).await.unwrap().unwrap();
    timeout(100, dont_fire_rx.next()).await.unwrap_err();
}

#[wasm_bindgen_test]
async fn on_track_disabled_works() {
    let track = get_audio_track().await;

    let track_clone = track.clone();
    let (test_tx, test_rx) = oneshot::channel();
    track.on_disabled(
        Closure::once_into_js(move || {
            assert!(!track_clone.js_enabled());
            test_tx.send(()).unwrap();
        })
        .into(),
    );

    let (dont_fire_tx, mut dont_fire_rx) = mpsc::unbounded();
    let dont_fire = || {
        let tx = dont_fire_tx.clone();
        Closure::once_into_js(move || {
            tx.unbounded_send(()).unwrap();
        })
        .into()
    };
    track.on_muted(dont_fire());
    track.on_unmuted(dont_fire());
    track.on_enabled(dont_fire());
    track.on_stopped(dont_fire());

    assert!(!track.muted());
    assert!(track.js_enabled());
    track.set_enabled(false);
    assert!(!track.muted());

    timeout(100, test_rx).await.unwrap().unwrap();
    timeout(100, dont_fire_rx.next()).await.unwrap_err();
}

#[wasm_bindgen_test]
async fn on_track_unmuted_works() {
    let track = get_audio_track().await;

    let track_clone = track.clone();
    let (test_tx, test_rx) = oneshot::channel();
    track.on_unmuted(
        Closure::once_into_js(move || {
            assert!(!track_clone.muted());
            test_tx.send(()).unwrap();
        })
        .into(),
    );

    let (dont_fire_tx, mut dont_fire_rx) = mpsc::unbounded();
    let dont_fire = || {
        let tx = dont_fire_tx.clone();
        Closure::once_into_js(move || {
            tx.unbounded_send(()).unwrap();
        })
        .into()
    };
    track.on_disabled(dont_fire());
    track.on_enabled(dont_fire());
    track.on_stopped(dont_fire());

    track.set_muted(true);
    assert!(track.js_enabled());
    assert!(track.muted());
    track.set_muted(false);
    assert!(track.js_enabled());
    assert!(!track.muted());

    timeout(100, test_rx).await.unwrap().unwrap();
    timeout(100, dont_fire_rx.next()).await.unwrap_err();
}

#[wasm_bindgen_test]
async fn on_track_muted_works() {
    let track = get_audio_track().await;

    let track_clone = track.clone();
    let (test_tx, test_rx) = oneshot::channel();
    track.on_muted(
        Closure::once_into_js(move || {
            assert!(track_clone.muted());
            test_tx.send(()).unwrap();
        })
        .into(),
    );

    let (dont_fire_tx, mut dont_fire_rx) = mpsc::unbounded();
    let dont_fire = || {
        let tx = dont_fire_tx.clone();
        Closure::once_into_js(move || {
            tx.unbounded_send(()).unwrap();
        })
        .into()
    };
    track.on_unmuted(dont_fire());
    track.on_disabled(dont_fire());
    track.on_enabled(dont_fire());
    track.on_stopped(dont_fire());

    assert!(track.js_enabled());
    assert!(!track.muted());
    track.set_muted(true);
    assert!(track.js_enabled());

    timeout(100, test_rx).await.unwrap().unwrap();
    timeout(100, dont_fire_rx.next()).await.unwrap_err();
}
