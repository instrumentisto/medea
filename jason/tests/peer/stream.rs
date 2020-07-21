use medea_client_api_proto::TrackId;
use medea_jason::peer::PeerMediaStream;
use wasm_bindgen_test::*;
use web_sys::MediaStreamTrack as SysMediaStreamTrack;

use crate::{get_audio_track, wait_and_check_test_result};

#[wasm_bindgen_test]
async fn on_track_added_works() {
    let stream = PeerMediaStream::new();
    let stream_handle = stream.new_handle();

    let (on_track_added, test_result_on_added) =
        js_callback!(|track: SysMediaStreamTrack| {
            cb_assert_eq!(track.kind(), "audio".to_string());
        });

    stream_handle.on_track_added(on_track_added.into()).unwrap();
    stream.add_track(TrackId(1), get_audio_track().await);

    assert!(stream_handle.has_active_audio().unwrap());
    assert!(!stream_handle.has_active_video().unwrap());
    wait_and_check_test_result(test_result_on_added, || {}).await;
}

#[wasm_bindgen_test]
async fn on_track_enabled_works() {
    let stream = PeerMediaStream::new();
    let track = get_audio_track().await;
    stream.add_track(TrackId(1), track.clone());

    let (on_track_enabled, test_result_on_enabled) =
        js_callback!(|track: SysMediaStreamTrack| {
            cb_assert_eq!(track.kind(), "audio".to_string());
        });
    let handle = stream
        .new_handle();

    handle
        .on_track_enabled(on_track_enabled.into())
        .unwrap();

    assert!(handle.has_active_audio().unwrap());
    track.set_enabled(false);
    assert!(!handle.has_active_audio().unwrap());
    track.set_enabled(true);
    assert!(handle.has_active_audio().unwrap());

    wait_and_check_test_result(test_result_on_enabled, || {}).await;
}

#[wasm_bindgen_test]
async fn on_track_disabled_works() {
    let stream = PeerMediaStream::new();
    let track = get_audio_track().await;
    stream.add_track(TrackId(1), track.clone());

    let (on_track_disabled, test_result_on_disabled) =
        js_callback!(|track: SysMediaStreamTrack| {
            cb_assert_eq!(track.kind(), "audio".to_string());
        });
    stream
        .new_handle()
        .on_track_disabled(on_track_disabled.into())
        .unwrap();

    track.set_enabled(false);

    wait_and_check_test_result(test_result_on_disabled, || {}).await;
}
