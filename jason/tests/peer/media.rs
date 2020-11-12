#![cfg(target_arch = "wasm32")]

use std::{convert::TryFrom, mem, rc::Rc};

use futures::channel::mpsc;
use medea_client_api_proto::{TrackId, TrackPatchEvent};
use medea_jason::{
    media::{LocalTracksConstraints, MediaManager, RecvConstraints},
    peer::{
        media_exchange_state, Disableable, LocalStreamUpdateCriteria,
        MediaConnections, RtcPeerConnection, SimpleTracksRequest,
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
        Rc::new(RtcPeerConnection::new(Vec::new(), false).unwrap()),
        tx,
    );
    let (audio_track, video_track) = get_test_unrequired_tracks();
    let audio_track_id = audio_track.id;
    let video_track_id = video_track.id;
    media_connections
        .create_tracks(
            vec![audio_track, video_track],
            &get_media_stream_settings(enabled_audio, enabled_video).into(),
            &RecvConstraints::default(),
        )
        .unwrap();
    let request = media_connections
        .get_tracks_request(LocalStreamUpdateCriteria::all())
        .unwrap();
    let caps = SimpleTracksRequest::try_from(request).unwrap();
    let manager = Rc::new(MediaManager::default());
    let tracks = manager.get_tracks(&caps).await.unwrap();

    media_connections
        .insert_local_tracks(
            &caps
                .parse_tracks(tracks.into_iter().map(|(t, _)| t).collect())
                .unwrap(),
        )
        .await
        .unwrap();

    media_connections
        .get_sender_by_id(audio_track_id)
        .unwrap()
        .media_exchange_state_transition_to(media_exchange_state::Stable::from(
            enabled_audio,
        ))
        .unwrap();
    media_connections
        .get_sender_by_id(video_track_id)
        .unwrap()
        .media_exchange_state_transition_to(media_exchange_state::Stable::from(
            enabled_video,
        ))
        .unwrap();

    (media_connections, audio_track_id, video_track_id)
}

#[wasm_bindgen_test]
fn get_tracks_request1() {
    let (tx, rx) = mpsc::unbounded();
    mem::forget(rx);
    let media_connections = MediaConnections::new(
        Rc::new(RtcPeerConnection::new(Vec::new(), false).unwrap()),
        tx,
    );
    let (audio_track, video_track) = get_test_unrequired_tracks();
    media_connections
        .create_tracks(
            vec![audio_track, video_track],
            &local_constraints(true, true),
            &RecvConstraints::default(),
        )
        .unwrap();
    let request =
        media_connections.get_tracks_request(LocalStreamUpdateCriteria::all());
    assert!(request.is_some());
}

#[wasm_bindgen_test]
fn get_tracks_request2() {
    let (tx, rx) = mpsc::unbounded();
    mem::forget(rx);
    let media_connections = MediaConnections::new(
        Rc::new(RtcPeerConnection::new(Vec::new(), false).unwrap()),
        tx,
    );
    media_connections
        .create_tracks(
            Vec::new(),
            &LocalTracksConstraints::default(),
            &RecvConstraints::default(),
        )
        .unwrap();
    let request =
        media_connections.get_tracks_request(LocalStreamUpdateCriteria::all());
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
    use media_exchange_state::Stable::{Disabled, Enabled};

    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(true, true).await;

    let audio_track =
        media_connections.get_sender_by_id(audio_track_id).unwrap();
    let video_track =
        media_connections.get_sender_by_id(video_track_id).unwrap();

    assert!(!audio_track.is_general_disabled());
    assert!(!video_track.is_general_disabled());

    audio_track
        .media_exchange_state_transition_to(Disabled)
        .unwrap();
    media_connections
        .patch_tracks(vec![TrackPatchEvent {
            id: audio_track_id,
            enabled_general: Some(false),
            enabled_individual: Some(false),
        }])
        .await
        .unwrap();
    assert!(audio_track.is_general_disabled());
    assert!(!video_track.is_general_disabled());

    video_track
        .media_exchange_state_transition_to(Disabled)
        .unwrap();
    media_connections
        .patch_tracks(vec![TrackPatchEvent {
            id: video_track_id,
            enabled_general: Some(false),
            enabled_individual: Some(false),
        }])
        .await
        .unwrap();
    assert!(audio_track.is_general_disabled());
    assert!(video_track.is_general_disabled());

    audio_track
        .media_exchange_state_transition_to(Enabled)
        .unwrap();
    media_connections
        .patch_tracks(vec![TrackPatchEvent {
            id: audio_track_id,
            enabled_individual: Some(true),
            enabled_general: Some(true),
        }])
        .await
        .unwrap();
    assert!(!audio_track.is_general_disabled());
    assert!(video_track.is_general_disabled());

    video_track
        .media_exchange_state_transition_to(Enabled)
        .unwrap();
    media_connections
        .patch_tracks(vec![TrackPatchEvent {
            id: video_track_id,
            enabled_individual: Some(true),
            enabled_general: Some(true),
        }])
        .await
        .unwrap();
    assert!(!audio_track.is_general_disabled());
    assert!(!video_track.is_general_disabled());
}

#[wasm_bindgen_test]
async fn new_media_connections_with_disabled_audio_tracks() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(false, true).await;

    let audio_track =
        media_connections.get_sender_by_id(audio_track_id).unwrap();
    let video_track =
        media_connections.get_sender_by_id(video_track_id).unwrap();

    assert!(audio_track.is_general_disabled());
    assert!(!video_track.is_general_disabled());
}

#[wasm_bindgen_test]
async fn new_media_connections_with_disabled_video_tracks() {
    let (media_connections, audio_track_id, video_track_id) =
        get_test_media_connections(true, false).await;

    let audio_track =
        media_connections.get_sender_by_id(audio_track_id).unwrap();
    let video_track =
        media_connections.get_sender_by_id(video_track_id).unwrap();

    assert!(!audio_track.is_general_disabled());
    assert!(video_track.is_general_disabled());
}

/// Tests for [`Sender::update`] function.
///
/// This tests checks that [`TrackPatch`] works as expected.
mod sender_patch {
    use medea_jason::peer::Sender;

    use super::*;

    async fn get_sender() -> (Rc<Sender>, TrackId, MediaConnections) {
        let (media_connections, audio_track_id, _) =
            get_test_media_connections(true, false).await;

        let audio_track =
            media_connections.get_sender_by_id(audio_track_id).unwrap();

        (audio_track, audio_track_id, media_connections)
    }

    #[wasm_bindgen_test]
    async fn wrong_track_id() {
        let (sender, track_id, _media_connections) = get_sender().await;
        sender
            .update(&TrackPatchEvent {
                id: TrackId(track_id.0 + 100),
                enabled_individual: Some(false),
                enabled_general: Some(false),
            })
            .await;

        assert!(!sender.is_general_disabled());
    }

    #[wasm_bindgen_test]
    async fn disable() {
        let (sender, track_id, _media_connections) = get_sender().await;
        sender
            .update(&TrackPatchEvent {
                id: track_id,
                enabled_individual: Some(false),
                enabled_general: Some(false),
            })
            .await;

        assert!(sender.is_general_disabled());
    }

    #[wasm_bindgen_test]
    async fn enabled_enabled() {
        let (sender, track_id, _media_connections) = get_sender().await;
        sender
            .update(&TrackPatchEvent {
                id: track_id,
                enabled_individual: Some(true),
                enabled_general: Some(true),
            })
            .await;

        assert!(!sender.is_general_disabled());
    }

    #[wasm_bindgen_test]
    async fn disable_disabled() {
        let (sender, track_id, _media_connections) = get_sender().await;
        sender
            .update(&TrackPatchEvent {
                id: track_id,
                enabled_individual: Some(false),
                enabled_general: Some(false),
            })
            .await;
        assert!(sender.is_general_disabled());

        sender
            .update(&TrackPatchEvent {
                id: track_id,
                enabled_individual: Some(false),
                enabled_general: Some(false),
            })
            .await;

        assert!(sender.is_general_disabled());
    }

    #[wasm_bindgen_test]
    async fn empty_patch() {
        let (sender, track_id, _media_connections) = get_sender().await;
        sender
            .update(&TrackPatchEvent {
                id: track_id,
                enabled_individual: None,
                enabled_general: None,
            })
            .await;

        assert!(!sender.is_general_disabled());
    }
}

mod receiver_patch {
    use medea_client_api_proto::{AudioSettings, MediaType, MemberId};
    use medea_jason::{
        media::RecvConstraints,
        peer::{PeerEvent, Receiver},
    };

    use super::*;

    const TRACK_ID: TrackId = TrackId(0);
    const MID: &str = "mid";
    const SENDER_ID: &str = "sender";

    fn get_receiver() -> (Rc<Receiver>, mpsc::UnboundedReceiver<PeerEvent>) {
        let (tx, rx) = mpsc::unbounded();
        let media_connections = MediaConnections::new(
            Rc::new(RtcPeerConnection::new(Vec::new(), false).unwrap()),
            tx,
        );
        let recv = Receiver::new(
            &media_connections,
            TRACK_ID,
            MediaType::Audio(AudioSettings { required: true }).into(),
            MemberId(SENDER_ID.to_string()),
            Some(MID.to_string()),
            &RecvConstraints::default(),
        );

        (Rc::new(recv), rx)
    }

    #[wasm_bindgen_test]
    async fn wrong_track_id() {
        let (receiver, _tx) = get_receiver();
        receiver.update(&TrackPatchEvent {
            id: TrackId(TRACK_ID.0 + 100),
            enabled_individual: Some(false),
            enabled_general: Some(false),
        });

        assert!(!receiver.is_general_disabled());
    }

    #[wasm_bindgen_test]
    async fn disable() {
        let (receiver, _tx) = get_receiver();
        receiver.update(&TrackPatchEvent {
            id: TRACK_ID,
            enabled_individual: Some(false),
            enabled_general: Some(false),
        });

        assert!(receiver.is_general_disabled());
    }

    #[wasm_bindgen_test]
    async fn enabled_enabled() {
        let (receiver, _tx) = get_receiver();
        receiver.update(&TrackPatchEvent {
            id: TRACK_ID,
            enabled_individual: Some(true),
            enabled_general: Some(true),
        });

        assert!(!receiver.is_general_disabled());
    }

    #[wasm_bindgen_test]
    async fn disable_disabled() {
        let (receiver, _tx) = get_receiver();
        receiver.update(&TrackPatchEvent {
            id: TRACK_ID,
            enabled_individual: Some(false),
            enabled_general: Some(false),
        });
        assert!(receiver.is_general_disabled());

        receiver.update(&TrackPatchEvent {
            id: TRACK_ID,
            enabled_individual: Some(false),
            enabled_general: Some(false),
        });

        assert!(receiver.is_general_disabled());
    }

    #[wasm_bindgen_test]
    async fn empty_patch() {
        let (receiver, _tx) = get_receiver();
        receiver.update(&TrackPatchEvent {
            id: TRACK_ID,
            enabled_individual: None,
            enabled_general: None,
        });

        assert!(!receiver.is_general_disabled());
    }
}
