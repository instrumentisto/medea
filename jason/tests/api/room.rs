#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use futures::channel::mpsc;
use medea_client_api_proto::{Event, IceServer, PeerId};
use medea_jason::{
    api::Room,
    media::MediaManager,
    peer::{MockPeerRepository, PeerConnection, PeerEvent},
    rpc::MockRpcClient,
    AudioTrackConstraints, MediaStreamConstraints,
};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use crate::{
    get_test_tracks, resolve_after, wait_and_check_test_result, MockNavigator,
};

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
    rpc.expect_set_close_reason().return_const(());

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
    rpc.expect_set_close_reason().return_const(());

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
    let (cb, test_rx) = js_callback!(|err: js_sys::Error| {
        cb_assert_eq!(
            err.to_string().as_string().unwrap(),
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

    wait_and_check_test_result(test_rx).await;
}

// Tests Room::inject_local_stream for existing PeerConnection.
// Setup:
//     1. Create Room.
//     2. Set `on_failed_local_stream` callback.
//     3. Acquire audio track.
//     4. Acquire local media stream without video track.
//     5. Inject local stream to Room and try change stream into existing
// peer. Assertions:
//     1. Invoking `on_failed_local_stream` callback.
#[wasm_bindgen_test]
async fn error_inject_invalid_local_stream_into_room_on_exists_peer() {
    let (cb, test_result) = js_callback!(|err: js_sys::Error| {
        cb_assert_eq!(
            err.to_string().as_string().unwrap(),
            "Error: provided MediaStream was expected to have single video \
             track"
        );
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

    wait_and_check_test_result(test_result).await;
}

#[wasm_bindgen_test]
async fn error_get_local_stream_on_new_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, _peer) = get_test_room_and_new_peer(event_rx, true, true);

    let room_handle = room.new_handle();

    let (cb, test_result) = js_callback!(|err: js_sys::Error| {
        cb_assert_eq!(
            err.to_string().as_string().unwrap(),
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

    wait_and_check_test_result(test_result).await;
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
    rpc.expect_set_close_reason().return_const(());
    let repo = Box::new(MockPeerRepository::new());
    let room = Room::new(Rc::new(rpc), repo);

    let room_handle = room.new_handle();
    match JsFuture::from(room_handle.join("token".to_string())).await {
        Ok(_) => assert!(
            false,
            "Not allowed join if `on_failed_local_stream` callback is not set"
        ),
        Err(_) => assert!(true),
    }
}

mod on_close_callback {
    //! Tests for `RoomHandle.on_close` JS side callback.

    use std::rc::Rc;

    use futures::channel::mpsc;
    use medea_client_api_proto::CloseReason as CloseByServerReason;
    use medea_jason::{
        api::Room,
        peer::MockPeerRepository,
        rpc::{ClientDisconnect, CloseReason, MockRpcClient},
    };
    use wasm_bindgen::{prelude::*, JsValue};
    use wasm_bindgen_test::*;

    use super::wait_and_check_test_result;

    #[wasm_bindgen(inline_js = "export function get_reason(closed) { return \
                                closed.reason; }")]
    extern "C" {
        fn get_reason(closed: &JsValue) -> String;
    }
    #[wasm_bindgen(inline_js = "export function \
                                get_is_closed_by_server(reason) { return \
                                reason.is_closed_by_server; }")]
    extern "C" {
        fn get_is_closed_by_server(reason: &JsValue) -> bool;
    }
    #[wasm_bindgen(inline_js = "export function get_is_err(reason) { return \
                                reason.is_err; }")]
    extern "C" {
        fn get_is_err(reason: &JsValue) -> bool;
    }

    /// Returns empty [`Room`] with mocks inside.
    fn get_room() -> Room {
        let mut rpc = MockRpcClient::new();
        let repo = Box::new(MockPeerRepository::new());

        let (_event_tx, event_rx) = mpsc::unbounded();
        rpc.expect_subscribe()
            .return_once(move || Box::pin(event_rx));
        rpc.expect_send_command().return_const(());
        rpc.expect_unsub().return_const(());
        rpc.expect_set_close_reason().return_const(());

        Room::new(Rc::new(rpc), repo)
    }

    /// Tests that JS side [`RoomHandle::on_close`] works.
    ///
    /// # Algorithm
    ///
    /// 1. Subscribe to [`RoomHandle::on_close`].
    ///
    /// 2. Call [`Room::close`] with [`CloseByServerReason::Finished`] reason.
    ///
    /// 3. Check that JS callback was called with this reason.
    #[wasm_bindgen_test]
    async fn closed_by_server() {
        let room = get_room();
        let mut room_handle = room.new_handle();

        let (cb, test_result) = js_callback!(|closed: JsValue| {
            cb_assert_eq!(get_reason(&closed), "Finished");
            cb_assert_eq!(get_is_closed_by_server(&closed), true);
            cb_assert_eq!(get_is_err(&closed), false);
        });
        room_handle.on_close(cb.into()).unwrap();

        room.close(CloseReason::ByServer(CloseByServerReason::Finished));
        wait_and_check_test_result(test_result).await;
    }

    /// Tests that [`RoomHandle::on_close`] will be called on unexpected
    /// [`Room`] drop.
    ///
    /// # Algorithm
    ///
    /// 1. Subscribe to [`RoomHandle::on_close`].
    ///
    /// 2. Drop [`Room`].
    ///
    /// 3. Check that JS callback was called with
    ///    `CloseReason::ByClient(ClosedByClientReason::
    /// RoomUnexpectedlyDropped`.
    #[wasm_bindgen_test]
    async fn unexpected_room_drop() {
        let room = get_room();
        let mut room_handle = room.new_handle();

        let (cb, test_result) = js_callback!(|closed: JsValue| {
            cb_assert_eq!(get_reason(&closed), "RoomUnexpectedlyDropped");
            cb_assert_eq!(get_is_err(&closed), true);
            cb_assert_eq!(get_is_closed_by_server(&closed), false);
        });
        room_handle.on_close(cb.into()).unwrap();

        std::mem::drop(room);
        wait_and_check_test_result(test_result).await;
    }

    /// Tests that [`RoomHandle::on_close`] will be called on closing by Jason.
    ///
    /// # Algorithm
    ///
    /// 1. Subscribe to [`RoomHandle::on_close`].
    ///
    /// 2. Call [`Room::close`] with [`CloseReason::ByClient`]
    ///
    /// 3. Check that JS callback was called with this [`CloseReason`].
    #[wasm_bindgen_test]
    async fn normal_close_by_client() {
        let room = get_room();
        let mut room_handle = room.new_handle();

        let (cb, test_result) = js_callback!(|closed: JsValue| {
            cb_assert_eq!(get_reason(&closed), "RoomUnexpectedlyDropped");
            cb_assert_eq!(get_is_err(&closed), false);
            cb_assert_eq!(get_is_closed_by_server(&closed), false);
        });
        room_handle.on_close(cb.into()).unwrap();

        room.close(CloseReason::ByClient {
            reason: ClientDisconnect::RoomUnexpectedlyDropped,
            is_err: false,
        });
        wait_and_check_test_result(test_result).await;
    }
}

mod rpc_close_reason_on_room_drop {
    //! Tests which checks that when [`Room`] is dropped the right close reason
    //! is provided to [`RpcClient`].

    use futures::channel::oneshot;
    use medea_jason::rpc::{ClientDisconnect, CloseReason};

    use super::*;

    /// Returns [`Room`] and [`oneshot::Receiver`] which will be resolved
    /// with [`RpcClient`]'s close reason ([`ClientDisconnect`]).
    async fn get_client() -> (Room, oneshot::Receiver<ClientDisconnect>) {
        let mut rpc = MockRpcClient::new();
        let repo = Box::new(MockPeerRepository::new());

        let (_event_tx, event_rx) = mpsc::unbounded();
        rpc.expect_subscribe()
            .return_once(move || Box::pin(event_rx));
        rpc.expect_send_command().return_const(());
        rpc.expect_unsub().return_const(());
        let (test_tx, test_rx) = oneshot::channel();
        rpc.expect_set_close_reason().return_once(move |reason| {
            test_tx.send(reason).unwrap();
        });
        let room = Room::new(Rc::new(rpc), repo);
        (room, test_rx)
    }

    /// Tests that [`Room`] sets right [`ClientDisconnect`] close reason on
    /// UNexpected drop.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcClient::set_close_reason`].
    ///
    /// 2. Drop [`Room`].
    ///
    /// 3. Check that close reason provided into [`RpcClient::set_close_reason`]
    ///    is [`ClientDisconnect::RoomUnexpectedlyDropped`].
    #[wasm_bindgen_test]
    async fn set_default_close_reason_on_drop() {
        let (room, test_rx) = get_client().await;

        std::mem::drop(room);

        let close_reason = test_rx.await.unwrap();
        assert_eq!(
            &close_reason,
            &ClientDisconnect::RoomUnexpectedlyDropped,
            "Room sets RPC close reason '{:?} instead of \
             'RoomUnxpectedlyDropped'.",
            close_reason,
        )
    }

    /// Tests that [`Room`] sets right [`ClientDisconnect`] close reason on
    /// expected drop.
    ///
    /// # Algorithm
    ///
    /// 1. Mock [`RpcClient::set_close_reason`].
    ///
    /// 2. Close [`Room`] with [`Room::close`] with
    ///    [`ClientDisconnect::RoomClosed`] as close reason.
    ///
    /// 3. Check that close reason provided into [`RpcClient::set_close_reason`]
    ///    is [`ClientDisconnect::RoomClosed`].
    #[wasm_bindgen_test]
    async fn sets_provided_close_reason_on_drop() {
        let (room, test_rx) = get_client().await;
        room.close(CloseReason::ByClient {
            reason: ClientDisconnect::RoomClosed,
            is_err: false,
        });

        let close_reason = test_rx.await.unwrap();
        assert_eq!(
            &close_reason,
            &ClientDisconnect::RoomClosed,
            "Room sets RPC close reason '{:?}' instead of 'RoomClosed.",
            close_reason,
        );
    }
}
