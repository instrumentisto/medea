#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use futures::channel::{mpsc, oneshot};
use medea_client_api_proto::{Event, IceServer, PeerId};
use medea_jason::{
    api::Room,
    media::MediaManager,
    peer::{MockPeerRepository, PeerConnection, PeerEvent},
    rpc::MockRpcClient,
    AudioTrackConstraints, MediaStreamConstraints,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use crate::{get_test_tracks, resolve_after};

wasm_bindgen_test_configure!(run_in_browser);

fn get_test_room_and_exist_peer(
    count_gets_peer: usize,
) -> (Room, Rc<PeerConnection>) {
    let mut rpc = MockRpcClient::new();
    let mut repo = Box::new(MockPeerRepository::new());
    let (tx, _rx) = mpsc::unbounded();
    let peer = Rc::new(
        PeerConnection::new(
            PeerId(1),
            tx,
            vec![],
            Rc::new(MediaManager::default()),
            true,
            true,
        )
        .unwrap(),
    );

    let (_, event_rx) = mpsc::unbounded();
    let peer_clone = Rc::clone(&peer);
    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    repo.expect_get_all()
        .times(count_gets_peer)
        .returning_st(move || vec![Rc::clone(&peer_clone)]);
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), repo);
    (room, peer)
}

#[wasm_bindgen_test]
async fn mute_unmute_audio() {
    let (room, peer) = get_test_room_and_exist_peer(2);
    let (audio_track, video_track) = get_test_tracks();

    peer.get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();
    let handle = room.new_handle();
    assert!(handle.mute_audio().is_ok());
    assert!(!peer.is_send_audio_enabled());
    assert!(handle.unmute_audio().is_ok());
    assert!(peer.is_send_audio_enabled());
}

#[wasm_bindgen_test]
async fn mute_unmute_video() {
    let (room, peer) = get_test_room_and_exist_peer(2);
    let (audio_track, video_track) = get_test_tracks();

    peer.get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();

    let handle = room.new_handle();
    assert!(handle.mute_video().is_ok());
    assert!(!peer.is_send_video_enabled());
    assert!(handle.unmute_video().is_ok());
    assert!(peer.is_send_video_enabled());
}

fn get_test_room_and_new_peer(
    event_rx: mpsc::UnboundedReceiver<Event>,
    with_enabled_audio: bool,
    with_enabled_video: bool,
) -> (Room, Rc<PeerConnection>) {
    let mut rpc = MockRpcClient::new();
    let mut repo = Box::new(MockPeerRepository::new());

    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    repo.expect_get_all().returning(|| vec![]);
    let (tx, _rx) = mpsc::unbounded();
    let peer = Rc::new(
        PeerConnection::new(
            PeerId(1),
            tx,
            vec![],
            Rc::new(MediaManager::default()),
            with_enabled_audio,
            with_enabled_video,
        )
        .unwrap(),
    );
    let peer_clone = Rc::clone(&peer);
    repo.expect_create_peer()
        .withf(
            move |id: &PeerId,
                  _ice_servers: &Vec<IceServer>,
                  _peer_events_sender: &mpsc::UnboundedSender<PeerEvent>,
                  enabled_audio: &bool,
                  enabled_video: &bool| {
                *id == PeerId(1)
                    && *enabled_audio == with_enabled_audio
                    && *enabled_video == with_enabled_video
            },
        )
        .return_once_st(move |_, _, _, _, _| Ok(peer_clone));
    rpc.expect_send_command().return_const(());
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), repo);
    (room, peer)
}

#[wasm_bindgen_test]
async fn mute_audio_room_before_init_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, peer) = get_test_room_and_new_peer(event_rx, false, true);
    let (audio_track, video_track) = get_test_tracks();

    room.new_handle().mute_audio().unwrap();
    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            sdp_offer: None,
            tracks: vec![audio_track, video_track],
            ice_servers: vec![],
        })
        .unwrap();

    resolve_after(500).await.unwrap();

    assert!(peer.is_send_video_enabled());
    assert!(!peer.is_send_audio_enabled());
}

#[wasm_bindgen_test]
async fn mute_video_room_before_init_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, peer) = get_test_room_and_new_peer(event_rx, true, false);
    let (audio_track, video_track) = get_test_tracks();

    room.new_handle().mute_video().unwrap();
    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            sdp_offer: None,
            tracks: vec![audio_track, video_track],
            ice_servers: vec![],
        })
        .unwrap();

    resolve_after(500).await.unwrap();

    assert!(peer.is_send_audio_enabled());
    assert!(!peer.is_send_video_enabled());
}

// Tests Room::inject_local_stream for create new PeerConnection.
// Setup:
//     1. Create Room.
//     2. Set `on_failed_local_stream` callback.
//     3. Acquire audio track.
//     4. Acquire local media stream without video track.
//     5. Inject local stream to Room.
//     6. Try create PeerConnection with injected stream.
// Assertions:
//     1. Invoking `on_failed_local_stream` callback.
#[wasm_bindgen_test]
async fn error_inject_invalid_local_stream_into_new_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, _peer) = get_test_room_and_new_peer(event_rx, true, true);

    let room_handle = room.new_handle();
    let (done, wait) = oneshot::channel();
    let cb = Closure::once_into_js(move |err: js_sys::Error| {
        assert_eq!(
            err.to_string(),
            "Error: provided MediaStream was expected to have single video \
             track"
        );
        done.send(()).unwrap();
    });
    room_handle.on_failed_local_stream(cb.into()).unwrap();

    let (audio_track, video_track) = get_test_tracks();

    let media_manager = MediaManager::default();
    let mut constraints = MediaStreamConstraints::new();
    let audio_constraints = AudioTrackConstraints::new();
    constraints.audio(audio_constraints);
    let (stream, _) = media_manager.get_stream(constraints).await.unwrap();

    room_handle.inject_local_stream(stream).unwrap();

    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            sdp_offer: None,
            tracks: vec![audio_track, video_track],
            ice_servers: vec![],
        })
        .unwrap();

    wait.await.unwrap();
}

// Tests Room::inject_local_stream for existing PeerConnection.
// Setup:
//     1. Create Room.
//     2. Set `on_failed_local_stream` callback.
//     3. Acquire audio track.
//     4. Acquire local media stream without video track.
//     5. Inject local stream to Room and try change stream into existing peer.
// Assertions:
//     1. Invoking `on_failed_local_stream` callback.
#[wasm_bindgen_test]
async fn error_inject_invalid_local_stream_into_room_on_exists_peer() {
    let (done, wait) = oneshot::channel();
    let cb = Closure::once_into_js(move |err: js_sys::Error| {
        assert_eq!(
            err.to_string(),
            "Error: provided MediaStream was expected to have single video \
             track"
        );
        done.send(()).unwrap();
    });
    let (room, peer) = get_test_room_and_exist_peer(1);
    let (audio_track, video_track) = get_test_tracks();
    peer.get_offer(vec![audio_track, video_track], None)
        .await
        .unwrap();

    let media_manager = MediaManager::default();
    let mut constraints = MediaStreamConstraints::new();
    let audio_constraints = AudioTrackConstraints::new();
    constraints.audio(audio_constraints);
    let (stream, _) = media_manager.get_stream(constraints).await.unwrap();
    let room_handle = room.new_handle();
    room_handle.on_failed_local_stream(cb.into()).unwrap();
    room_handle.inject_local_stream(stream).unwrap();

    wait.await.unwrap();
}
