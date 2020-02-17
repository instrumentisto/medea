#![cfg(target_arch = "wasm32")]

mod media;

use std::rc::Rc;

use futures::{channel::mpsc, StreamExt as _};
use medea_client_api_proto::{IceConnectionState, PeerId};
use medea_jason::{
    media::MediaManager,
    peer::{PeerConnection, PeerEvent},
};
use wasm_bindgen_test::*;

use crate::{get_test_tracks, resolve_after};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn mute_unmute_audio() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        vec![],
        manager,
        true.into(),
        true.into(),
        false,
    )
    .unwrap();

    peer.get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();

    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());

    peer.toggle_send_audio(false.into());
    assert!(!peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());

    peer.toggle_send_audio(true.into());
    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());
}

#[wasm_bindgen_test]
async fn mute_unmute_video() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        vec![],
        manager,
        true.into(),
        true.into(),
        false,
    )
    .unwrap();
    peer.get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();

    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());

    peer.toggle_send_video(false.into());
    assert!(peer.is_send_audio_enabled());
    assert!(!peer.is_send_video_enabled());

    peer.toggle_send_video(true.into());
    assert!(peer.is_send_audio_enabled());
    assert!(peer.is_send_video_enabled());
}

#[wasm_bindgen_test]
async fn new_with_mute_audio() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        vec![],
        manager,
        false.into(),
        true.into(),
        false,
    )
    .unwrap();

    peer.get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();
    assert!(!peer.is_send_audio_enabled());

    assert!(peer.is_send_video_enabled());
}

#[wasm_bindgen_test]
async fn new_with_mute_video() {
    let (tx, _rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(
        PeerId(1),
        tx,
        vec![],
        manager,
        true.into(),
        false.into(),
        false,
    )
    .unwrap();
    peer.get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();

    assert!(peer.is_send_audio_enabled());
    assert!(!peer.is_send_video_enabled());
}

#[wasm_bindgen_test]
async fn add_candidates_to_answerer_before_offer() {
    let (tx1, rx1) = mpsc::unbounded();
    let (tx2, _) = mpsc::unbounded();

    let manager = Rc::new(MediaManager::default());
    let pc1 = PeerConnection::new(
        PeerId(1),
        tx1,
        vec![],
        Rc::clone(&manager),
        true.into(),
        true.into(),
        false,
    )
    .unwrap();

    let pc2 = PeerConnection::new(
        PeerId(2),
        tx2,
        vec![],
        manager,
        true.into(),
        true.into(),
        false,
    )
    .unwrap();
    let (audio_track, video_track) = get_test_tracks();
    let offer = pc1
        .get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();

    handle_ice_candidates(rx1, &pc2, 1).await;
    // assert that pc2 has buffered candidates
    assert!(pc2.candidates_buffer_len() > 0);
    // then set its remote description
    pc2.process_offer(offer, vec![], None).await.unwrap();

    // and assert that buffer was flushed
    assert_eq!(pc2.candidates_buffer_len(), 0);
}

#[wasm_bindgen_test]
async fn add_candidates_to_offerer_before_answer() {
    let (tx1, _) = mpsc::unbounded();
    let (tx2, rx2) = mpsc::unbounded();

    let manager = Rc::new(MediaManager::default());
    let pc1 = Rc::new(
        PeerConnection::new(
            PeerId(1),
            tx1,
            vec![],
            Rc::clone(&manager),
            true.into(),
            true.into(),
            false,
        )
        .unwrap(),
    );
    let pc2 = Rc::new(
        PeerConnection::new(
            PeerId(2),
            tx2,
            vec![],
            manager,
            true.into(),
            true.into(),
            false,
        )
        .unwrap(),
    );

    let (audio_track, video_track) = get_test_tracks();
    let offer = pc1
        .get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();
    let answer = pc2.process_offer(offer, vec![], None).await.unwrap();

    handle_ice_candidates(rx2, &pc1, 1).await;

    // assert that pc1 has buffered candidates
    assert!(pc1.candidates_buffer_len() > 0);
    pc1.set_remote_answer(answer).await.unwrap();
    // assert that pc1 has buffered candidates got fulshed
    assert_eq!(pc1.candidates_buffer_len(), 0);
}

#[wasm_bindgen_test]
async fn normal_exchange_of_candidates() {
    let (tx1, rx1) = mpsc::unbounded();
    let (tx2, rx2) = mpsc::unbounded();

    let manager = Rc::new(MediaManager::default());
    let peer1 = PeerConnection::new(
        PeerId(1),
        tx1,
        vec![],
        Rc::clone(&manager),
        true.into(),
        true.into(),
        false,
    )
    .unwrap();
    let peer2 = PeerConnection::new(
        PeerId(2),
        tx2,
        vec![],
        manager,
        true.into(),
        true.into(),
        false,
    )
    .unwrap();
    let (audio_track, video_track) = get_test_tracks();

    let offer = peer1
        .get_offer(vec![audio_track.clone(), video_track.clone()], None)
        .await
        .unwrap();
    let answer = peer2
        .process_offer(offer, vec![audio_track, video_track], None)
        .await
        .unwrap();
    peer1.set_remote_answer(answer).await.unwrap();

    resolve_after(500).await.unwrap();

    handle_ice_candidates(rx1, &peer2, 2).await;
    handle_ice_candidates(rx2, &peer1, 1).await;
}

async fn handle_ice_candidates(
    mut candidates_rx: mpsc::UnboundedReceiver<PeerEvent>,
    peer: &PeerConnection,
    count: u8,
) {
    let mut cnt = 0;

    while let Some(event) = candidates_rx.next().await {
        match event {
            PeerEvent::IceCandidateDiscovered {
                peer_id: _,
                candidate,
                sdp_m_line_index,
                sdp_mid,
            } => {
                peer.add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                    .await
                    .unwrap();

                cnt += 1;
                if cnt == count {
                    break;
                }
            }
            PeerEvent::NewLocalStream { .. } => {}
            _ => unreachable!(),
        }
    }
}

#[wasm_bindgen_test]
async fn send_event_on_new_local_stream() {
    let (tx, mut rx) = mpsc::unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let id = PeerId(1);
    let peer = PeerConnection::new(
        id,
        tx,
        vec![],
        manager,
        true.into(),
        false.into(),
        false,
    )
    .unwrap();
    peer.get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();

    while let Some(event) = rx.next().await {
        match event {
            PeerEvent::NewLocalStream { peer_id, .. } => {
                assert_eq!(peer_id, id);
                break;
            }
            _ => {}
        }
    }
}

/// Setup signalling between two peers and wait for:
/// 1. `IceConnectionState::Checking` from both peers.
/// 2. `IceConnectionState::Connected` from both peers.
#[wasm_bindgen_test]
async fn ice_connection_state_changed_is_emitted() {
    let (tx1, rx1) = mpsc::unbounded();
    let (tx2, rx2) = mpsc::unbounded();

    let manager = Rc::new(MediaManager::default());
    let peer1 = PeerConnection::new(
        PeerId(1),
        tx1,
        vec![],
        Rc::clone(&manager),
        true.into(),
        true.into(),
        false,
    )
    .unwrap();
    let peer2 = PeerConnection::new(
        PeerId(2),
        tx2,
        vec![],
        manager,
        true.into(),
        true.into(),
        false,
    )
    .unwrap();
    let (audio_track, video_track) = get_test_tracks();

    let offer = peer1
        .get_offer(vec![audio_track.clone(), video_track.clone()], None)
        .await
        .unwrap();
    let answer = peer2
        .process_offer(offer, vec![audio_track, video_track], None)
        .await
        .unwrap();
    peer1.set_remote_answer(answer).await.unwrap();

    resolve_after(500).await.unwrap();

    let mut events = futures::stream::select(rx1, rx2);

    let mut checking1 = false;
    let mut checking2 = false;
    let mut connected1 = false;
    let mut connected2 = false;
    while let Some(event) = events.next().await {
        let event: PeerEvent = event;
        match event {
            PeerEvent::IceCandidateDiscovered {
                peer_id,
                candidate,
                sdp_m_line_index,
                sdp_mid,
            } => {
                if peer_id.0 == 1 {
                    peer2
                        .add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                        .await
                        .unwrap();
                } else {
                    peer1
                        .add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                        .await
                        .unwrap();
                }
            }
            PeerEvent::IceConnectionStateChanged {
                peer_id,
                ice_connection_state,
            } => match ice_connection_state {
                IceConnectionState::Checking => {
                    if peer_id.0 == 1 {
                        checking1 = true;
                    } else {
                        checking2 = true;
                    }
                }
                IceConnectionState::Connected => {
                    if peer_id.0 == 1 {
                        connected1 = true;
                    } else {
                        connected2 = true;
                    }
                }
                _ => {}
            },
            _ => {}
        };

        if checking1 && checking2 && connected1 && connected2 {
            break;
        }
    }
}
