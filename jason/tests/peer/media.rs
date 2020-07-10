#![cfg(target_arch = "wasm32")]

use std::{convert::TryFrom, mem, rc::Rc};

use futures::channel::mpsc;
use medea_client_api_proto::{PeerId, TrackId, TrackPatch};
use medea_jason::{
    media::{LocalStreamConstraints, MediaManager},
    peer::{
        MediaConnections, RtcPeerConnection, SimpleStreamRequest,
        StableMuteState,
    },
};
use wasm_bindgen_test::*;

use crate::{
    get_media_stream_settings, get_test_unrequired_tracks, local_constraints,
};

wasm_bindgen_test_configure!(run_in_browser);

async fn get_test_media_connections(
    enabled_audio: bool,
    enabled_video: bool,
) -> (MediaConnections, TrackId, TrackId) {
    let (tx, rx) = mpsc::unbounded();
    mem::forget(rx);
    let media_connections = MediaConnections::new(
        PeerId(0),
        Rc::new(RtcPeerConnection::new(Vec::new(), false).unwrap()),
        tx,
    );
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let audio_track_id = audio_track.id;
    let video_track_id = video_track.id;
    media_connections
        .create_tracks(
            vec![audio_track, video_track],
            &get_media_stream_settings(!enabled_audio, !enabled_video).into(),
        )
        .unwrap();
    let request = media_connections.get_stream_request().unwrap();
    let caps = SimpleStreamRequest::try_from(request).unwrap();
    let manager = Rc::new(MediaManager::default());
    let (stream, _) = manager.get_stream(&caps).await.unwrap();

    media_connections
        .insert_local_stream(&caps.parse_stream(stream).unwrap())
        .await
        .unwrap();

    media_connections
        .get_sender_by_id(audio_track_id)
        .unwrap()
        .mute_state_transition_to(StableMuteState::from(!enabled_audio))
        .unwrap();
    media_connections
        .get_sender_by_id(video_track_id)
        .unwrap()
        .mute_state_transition_to(StableMuteState::from(!enabled_video))
        .unwrap();

    (media_connections, audio_track_id, video_track_id)
}

#[wasm_bindgen_test]
fn get_stream_request1() {
    let (tx, rx) = mpsc::unbounded();
    mem::forget(rx);
    let media_connections = MediaConnections::new(
        PeerId(0),
        Rc::new(RtcPeerConnection::new(Vec::new(), false).unwrap()),
        tx,
    );
    let (audio_track, video_track) = get_test_unrequired_tracks();
    media_connections
        .create_tracks(
            vec![audio_track, video_track],
            &local_constraints(true, true),
        )
        .unwrap();
    let request = media_connections.get_stream_request();
    assert!(request.is_some());
}

#[wasm_bindgen_test]
fn get_stream_request2() {
    let (tx, rx) = mpsc::unbounded();
    mem::forget(rx);
    let media_connections = MediaConnections::new(
        PeerId(0),
        Rc::new(RtcPeerConnection::new(Vec::new(), false).unwrap()),
        tx,
    );
    media_connections
        .create_tracks(Vec::new(), &LocalStreamConstraints::default())
        .unwrap();
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

    let audio_track =
        media_connections.get_sender_by_id(audio_track_id).unwrap();
    let video_track =
        media_connections.get_sender_by_id(video_track_id).unwrap();

    assert!(!audio_track.is_muted());
    assert!(!video_track.is_muted());

    audio_track
        .mute_state_transition_to(StableMuteState::Muted)
        .unwrap();
    media_connections
        .update_senders(vec![TrackPatch {
            id: audio_track_id,
            is_muted: Some(true),
        }])
        .unwrap();
    assert!(audio_track.is_muted());
    assert!(!video_track.is_muted());

    video_track
        .mute_state_transition_to(StableMuteState::Muted)
        .unwrap();
    media_connections
        .update_senders(vec![TrackPatch {
            id: video_track_id,
            is_muted: Some(true),
        }])
        .unwrap();
    assert!(audio_track.is_muted());
    assert!(video_track.is_muted());

    audio_track
        .mute_state_transition_to(StableMuteState::NotMuted)
        .unwrap();
    media_connections
        .update_senders(vec![TrackPatch {
            id: audio_track_id,
            is_muted: Some(false),
        }])
        .unwrap();
    assert!(!audio_track.is_muted());
    assert!(video_track.is_muted());

    video_track
        .mute_state_transition_to(StableMuteState::NotMuted)
        .unwrap();
    media_connections
        .update_senders(vec![TrackPatch {
            id: video_track_id,
            is_muted: Some(false),
        }])
        .unwrap();
    assert!(!audio_track.is_muted());
    assert!(!video_track.is_muted());
}

#[wasm_bindgen_test]
async fn new_media_connections_with_disabled_audio_tracks() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(false, true).await;

    let audio_track =
        media_connections.get_sender_by_id(audio_track_id).unwrap();
    let video_track =
        media_connections.get_sender_by_id(video_track_id).unwrap();

    assert!(audio_track.is_muted());
    assert!(!video_track.is_muted());
}

#[wasm_bindgen_test]
async fn new_media_connections_with_disabled_video_tracks() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(true, false).await;

    let audio_track =
        media_connections.get_sender_by_id(audio_track_id).unwrap();
    let video_track =
        media_connections.get_sender_by_id(video_track_id).unwrap();

    assert!(!audio_track.is_muted());
    assert!(video_track.is_muted());
}

/// Tests for [`Sender::update`] function.
///
/// This tests checks that [`TrackPatch`] works as expected.
mod sender_patch {
    use medea_jason::peer::Sender;

    use super::*;

    async fn get_sender() -> (Rc<Sender>, TrackId) {
        let (media_connections, audio_track_id, _) =
            get_test_media_connections(true, false).await;

        let audio_track =
            media_connections.get_sender_by_id(audio_track_id).unwrap();

        (audio_track, audio_track_id)
    }

    #[wasm_bindgen_test]
    async fn wrong_track_id() {
        let (sender, track_id) = get_sender().await;
        sender.update(&TrackPatch {
            id: TrackId(track_id.0 + 100),
            is_muted: Some(true),
        });

        assert!(!sender.is_muted());
    }

    #[wasm_bindgen_test]
    async fn mute() {
        let (sender, track_id) = get_sender().await;
        sender.update(&TrackPatch {
            id: track_id,
            is_muted: Some(true),
        });

        assert!(sender.is_muted());
    }

    #[wasm_bindgen_test]
    async fn unmute_unmuted() {
        let (sender, track_id) = get_sender().await;
        sender.update(&TrackPatch {
            id: track_id,
            is_muted: Some(false),
        });

        assert!(!sender.is_muted());
    }

    #[wasm_bindgen_test]
    async fn mute_muted() {
        let (sender, track_id) = get_sender().await;
        sender.update(&TrackPatch {
            id: track_id,
            is_muted: Some(true),
        });
        assert!(sender.is_muted());

        sender.update(&TrackPatch {
            id: track_id,
            is_muted: Some(true),
        });

        assert!(sender.is_muted());
    }

    #[wasm_bindgen_test]
    async fn empty_patch() {
        let (sender, track_id) = get_sender().await;
        sender.update(&TrackPatch {
            id: track_id,
            is_muted: None,
        });

        assert!(!sender.is_muted());
    }
}
