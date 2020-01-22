#![cfg(target_arch = "wasm32")]

use std::{convert::TryFrom, rc::Rc};

use medea_client_api_proto::TrackId;
use medea_jason::{
    media::MediaManager,
    peer::{
        MediaConnections, RtcPeerConnection, SimpleStreamRequest,
        TransceiverDirection,
    },
};
use wasm_bindgen_test::*;

use crate::{get_test_tracks, peer::toggle_mute_tracks_updates};
use medea_jason::peer::{MutedState, TransceiverKind};

wasm_bindgen_test_configure!(run_in_browser);

async fn get_test_media_connections(
    enabled_audio: bool,
    enabled_video: bool,
) -> (MediaConnections, TrackId, TrackId) {
    let media_connections = MediaConnections::new(Rc::new(
        RtcPeerConnection::new(vec![], false).unwrap(),
    ));
    let (audio_track, video_track) =
        get_test_tracks(!enabled_audio, !enabled_video);
    let audio_track_id = audio_track.id;
    let video_track_id = video_track.id;
    media_connections
        .update_tracks(vec![audio_track, video_track])
        .unwrap();
    let request = media_connections.get_stream_request().unwrap();
    let caps = SimpleStreamRequest::try_from(request).unwrap();
    let manager = Rc::new(MediaManager::default());
    let (stream, _) = manager.get_stream(&caps).await.unwrap();

    media_connections
        .insert_local_stream(&caps.parse_stream(&stream).unwrap())
        .await
        .unwrap();

    media_connections
        .get_sender(audio_track_id)
        .unwrap()
        .change_muted_state(MutedState::from(!enabled_audio));
    media_connections
        .get_sender(video_track_id)
        .unwrap()
        .change_muted_state(MutedState::from(!enabled_video));

    (media_connections, audio_track_id, video_track_id)
}

#[wasm_bindgen_test]
fn get_stream_request() {
    let media_connections = MediaConnections::new(Rc::new(
        RtcPeerConnection::new(vec![], false).unwrap(),
    ));
    let (audio_track, video_track) = get_test_tracks(false, false);
    media_connections
        .update_tracks(vec![audio_track, video_track])
        .unwrap();
    let request = media_connections.get_stream_request();
    assert!(request.is_some());

    let media_connections = MediaConnections::new(Rc::new(
        RtcPeerConnection::new(vec![], false).unwrap(),
    ));
    media_connections.update_tracks(vec![]).unwrap();
    let request = media_connections.get_stream_request();
    assert!(request.is_none());
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
        get_test_media_connections(true, true).await;

    let audio_track = media_connections.get_sender(audio_track_id).unwrap();
    let video_track = media_connections.get_sender(video_track_id).unwrap();

    assert_eq!(audio_track.muted_state(), MutedState::Unmuted);
    assert_eq!(video_track.muted_state(), MutedState::Unmuted);

    audio_track.change_muted_state(MutedState::Muted);
    assert_eq!(audio_track.muted_state(), MutedState::Muted);
    assert_eq!(video_track.muted_state(), MutedState::Unmuted);

    video_track.change_muted_state(MutedState::Muted);
    assert_eq!(audio_track.muted_state(), MutedState::Muted);
    assert_eq!(video_track.muted_state(), MutedState::Muted);

    audio_track.change_muted_state(MutedState::Unmuted);
    assert_eq!(audio_track.muted_state(), MutedState::Unmuted);
    assert_eq!(video_track.muted_state(), MutedState::Muted);

    video_track.change_muted_state(MutedState::Unmuted);
    assert_eq!(audio_track.muted_state(), MutedState::Unmuted);
    assert_eq!(video_track.muted_state(), MutedState::Unmuted);
}

#[wasm_bindgen_test]
async fn new_media_connections_with_disabled_audio_tracks() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(false, true).await;

    let audio_track = media_connections.get_sender(audio_track_id).unwrap();
    let video_track = media_connections.get_sender(video_track_id).unwrap();

    assert_eq!(audio_track.muted_state(), MutedState::Muted);
    assert_eq!(video_track.muted_state(), MutedState::Unmuted);
}

#[wasm_bindgen_test]
async fn new_media_connections_with_disabled_video_tracks() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(true, false).await;

    let audio_track = media_connections.get_sender(audio_track_id).unwrap();
    let video_track = media_connections.get_sender(video_track_id).unwrap();

    assert_eq!(audio_track.muted_state(), MutedState::Unmuted);
    assert_eq!(video_track.muted_state(), MutedState::Muted);
}
