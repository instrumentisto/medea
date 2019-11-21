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
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use crate::{get_test_tracks, resolve_after, MockNavigator};
use medea_jason::api::RoomStorage;

wasm_bindgen_test_configure!(run_in_browser);

fn get_test_room_and_exist_peer(
    count_gets_peer: usize,
) -> (Room, Rc<PeerConnection>, Rc<RoomStorage>) {
    let mut rpc = MockRpcClient::new();
    let mut repo = Box::new(MockPeerRepository::new());
    let store = Rc::new(RoomStorage::new(Rc::new(MediaManager::default())));
    let (tx, _rx) = mpsc::unbounded();
    let peer = Rc::new(
        PeerConnection::new(PeerId(1), tx, vec![], true, true).unwrap(),
    );

    let (_, event_rx) = mpsc::unbounded();
    let peer_clone = Rc::clone(&peer);
    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    repo.expect_get_all()
        .times(count_gets_peer)
        .returning_st(move || vec![Rc::clone(&peer_clone)]);
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), repo, Rc::clone(&store));
    (room, peer, store)
}

#[wasm_bindgen_test]
async fn mute_unmute_audio() {
    let (room, peer, store) = get_test_room_and_exist_peer(2);
    let (audio_track, video_track) = get_test_tracks();

    peer.get_offer(vec![audio_track, video_track], store.as_ref())
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
    let (room, peer, store) = get_test_room_and_exist_peer(2);
    let (audio_track, video_track) = get_test_tracks();

    peer.get_offer(vec![audio_track, video_track], store.as_ref())
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
    let store = Rc::new(RoomStorage::new(Rc::new(MediaManager::default())));

    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    repo.expect_get_all().returning(|| vec![]);
    let (tx, _rx) = mpsc::unbounded();
    let peer = Rc::new(
        PeerConnection::new(
            PeerId(1),
            tx,
            vec![],
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

    let room = Room::new(Rc::new(rpc), repo, store);
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
        done.send(()).unwrap();
        assert_eq!(
            err.to_string(),
            "Error: provided MediaStream was expected to have single video \
             track"
        );
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
        done.send(()).unwrap();
        assert_eq!(
            err.to_string(),
            "Error: provided MediaStream was expected to have single video \
             track"
        );
    });
    let (room, peer, store) = get_test_room_and_exist_peer(1);
    let (audio_track, video_track) = get_test_tracks();
    peer.get_offer(vec![audio_track, video_track], store.as_ref())
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

#[wasm_bindgen_test]
async fn error_get_local_stream_on_new_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, _peer) = get_test_room_and_new_peer(event_rx, true, true);

    let room_handle = room.new_handle();
    let (done, wait) = oneshot::channel();
    let cb = Closure::once_into_js(move |err: js_sys::Error| {
        done.send(()).unwrap();
        assert_eq!(
            err.to_string(),
            "Error: MediaDevices.getUserMedia() failed: some error"
        );
    });
    room_handle.on_failed_local_stream(cb.into()).unwrap();

    let mock_navigator = MockNavigator::new();
    mock_navigator.error_get_user_media("some error".into());

    let (audio_track, video_track) = get_test_tracks();
    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            sdp_offer: None,
            tracks: vec![audio_track, video_track],
            ice_servers: vec![],
        })
        .unwrap();

    wait.await.unwrap();
    mock_navigator.stop();
}

// Tests Room::join without set `on_failed_local_stream` callback.
// Setup:
//     1. Create Room.
//     2. DO NOT set `on_failed_local_stream` callback.
//     3. Try join to Room.
// Assertions:
//     1. Room::join returns error.
#[wasm_bindgen_test]
async fn error_join_room_without_failed_stream_callback() {
    let (_, event_rx) = mpsc::unbounded();
    let mut rpc = MockRpcClient::new();
    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    rpc.expect_unsub().return_const(());
    let repo = Box::new(MockPeerRepository::new());
    let store = Rc::new(RoomStorage::new(Rc::new(MediaManager::default())));
    let room = Room::new(Rc::new(rpc), repo, store);

    let room_handle = room.new_handle();
    match JsFuture::from(room_handle.join("token".to_string())).await {
        Ok(_) => assert!(
            false,
            "Not allowed join if `on_failed_local_stream` callback is not set"
        ),
        Err(_) => assert!(true),
    }
}
