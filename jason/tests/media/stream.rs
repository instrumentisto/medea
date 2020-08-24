#![cfg(target_arch = "wasm32")]

use medea_jason::{
    media::MediaManager, DeviceVideoTrackConstraints, MediaStreamSettings,
};
use wasm_bindgen_test::*;
use web_sys::MediaStreamTrackState;

// TODO: do something with it
/// Assert that track is stopped when all strong refs are dropped.
// #[wasm_bindgen_test]
async fn track_autostop() {
    let media_manager = MediaManager::default();
    let mut caps = MediaStreamSettings::new();
    caps.device_video(DeviceVideoTrackConstraints::new());

    let (stream, is_new) = media_manager.get_stream(caps).await.unwrap();
    assert!(is_new);

    let mut tracks = stream.into_tracks();
    assert_eq!(1, tracks.len());
    let strong_track = tracks.pop().unwrap();
    let sys_track = Clone::clone(strong_track.as_ref());
    let weak_track = strong_track.downgrade();

    assert!(sys_track.ready_state() == MediaStreamTrackState::Live);
    drop(strong_track);
    assert!(sys_track.ready_state() == MediaStreamTrackState::Ended);
    assert!(!weak_track.can_be_upgraded());
}
