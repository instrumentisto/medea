#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use medea_client_api_proto::TrackId;
use medea_jason::{
    media::MediaManager,
    peer::{
        MediaConnections, RtcPeerConnection, TransceiverDirection,
        TransceiverKind,
    },
    utils::WasmErr,
};
use wasm_bindgen_test::*;

use crate::get_test_tracks;

wasm_bindgen_test_configure!(run_in_browser);

async fn get_test_media_connections(
    enabled_audio: bool,
    enabled_video: bool,
) -> Result<(MediaConnections, TrackId, TrackId), WasmErr> {
    let media_connections = MediaConnections::new(
        Rc::new(RtcPeerConnection::new(vec![]).unwrap()),
        enabled_audio,
        enabled_video,
    );
    let (audio_track, video_track) = get_test_tracks();
    let audio_track_id = audio_track.id;
    let video_track_id = video_track.id;
    let request = media_connections
        .update_tracks(vec![audio_track, video_track])
        .unwrap()
        .unwrap();
    let manager = Rc::new(MediaManager::default());
    let stream = manager.get_stream_by_request(request).await?;

    media_connections.insert_local_stream(&stream).await?;

    Ok((media_connections, audio_track_id, video_track_id))
}

// Tests MediaConnections::toggle_send_media function.
// Setup:
//     1. Create MediaConnections.
//     2. Acquire tracks.
// Assertions:
//     1. Calling toggle_send_media(audio, false) disables audio track.
//     2. Calling toggle_send_media(audio, true) enables audio track.
//     3. Calling toggle_send_media(video, false) disables video track.
//     4. Calling toggle_send_media(video, true) enables video track.
#[wasm_bindgen_test]
async fn disable_and_enable_all_tracks_in_media_manager() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(true, true).await.unwrap();

    let audio_track = media_connections
        .get_track_by_id(TransceiverDirection::Sendonly, audio_track_id)
        .unwrap();
    let video_track = media_connections
        .get_track_by_id(TransceiverDirection::Sendonly, video_track_id)
        .unwrap();

    assert!(audio_track.is_enabled());
    assert!(video_track.is_enabled());

    media_connections.toggle_send_media(TransceiverKind::Audio, false);
    assert!(!audio_track.is_enabled());
    assert!(video_track.is_enabled());

    media_connections.toggle_send_media(TransceiverKind::Video, false);
    assert!(!audio_track.is_enabled());
    assert!(!video_track.is_enabled());

    media_connections.toggle_send_media(TransceiverKind::Audio, true);
    assert!(audio_track.is_enabled());
    assert!(!video_track.is_enabled());

    media_connections.toggle_send_media(TransceiverKind::Video, true);
    assert!(audio_track.is_enabled());
    assert!(video_track.is_enabled());
}

#[wasm_bindgen_test]
async fn new_media_connections_with_disabled_audio_tracks() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(false, true).await.unwrap();

    let audio_track = media_connections
        .get_track_by_id(TransceiverDirection::Sendonly, audio_track_id)
        .unwrap();
    let video_track = media_connections
        .get_track_by_id(TransceiverDirection::Sendonly, video_track_id)
        .unwrap();

    assert!(!audio_track.is_enabled());
    assert!(video_track.is_enabled());
}

#[wasm_bindgen_test]
async fn new_media_connections_with_disabled_video_tracks() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(true, false).await.unwrap();

    let audio_track = media_connections
        .get_track_by_id(TransceiverDirection::Sendonly, audio_track_id)
        .unwrap();
    let video_track = media_connections
        .get_track_by_id(TransceiverDirection::Sendonly, video_track_id)
        .unwrap();

    assert!(audio_track.is_enabled());
    assert!(!video_track.is_enabled());
}
