#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use futures::channel::mpsc;
use medea_client_api_proto::{Command, Event, IceServer, PeerId};
use medea_jason::{
    api::Room,
    media::{AudioTrackConstraints, MediaManager, MediaStreamSettings},
    peer::{
        MockPeerRepository, PeerConnection, PeerEvent, StableMuteState,
        TransceiverKind,
    },
    rpc::MockRpcClient,
    utils::JasonError,
};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

use crate::{get_test_tracks, wait_and_check_test_result, MockNavigator};

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
            false,
        )
        .unwrap(),
    );

    let (event_tx, event_rx) = mpsc::unbounded();
    let peer_clone = Rc::clone(&peer);
    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    repo.expect_get_all()
        .times(count_gets_peer)
        .returning_st(move || vec![Rc::clone(&peer_clone)]);
    let peer_clone = Rc::clone(&peer);
    repo.expect_get()
        .returning_st(move |_| Some(Rc::clone(&peer_clone)));
    rpc.expect_unsub().return_const(());
    rpc.expect_set_close_reason().return_const(());
    rpc.expect_send_command().returning(move |cmd| match cmd {
        Command::UpdateTracks {
            peer_id,
            tracks_patches,
        } => {
            event_tx
                .unbounded_send(Event::TracksUpdated {
                    peer_id,
                    tracks_patches,
                })
                .unwrap();
        }
        _ => (),
    });

    let room = Room::new(Rc::new(rpc), repo);
    (room, peer)
}

#[wasm_bindgen_test]
async fn mute_unmute_audio() {
    let (room, peer) = get_test_room_and_exist_peer(6);
    let (audio_track, video_track) = get_test_tracks(false, false);

    peer.get_offer(vec![audio_track, video_track], None, false)
        .await
        .unwrap();
    let handle = room.new_handle();
    assert!(JsFuture::from(handle.mute_audio()).await.is_ok());
    assert!(!peer.is_send_audio_enabled());
    assert!(JsFuture::from(handle.unmute_audio()).await.is_ok());
    assert!(peer.is_send_audio_enabled());
}

#[wasm_bindgen_test]
async fn mute_unmute_video() {
    let (room, peer) = get_test_room_and_exist_peer(6);
    let (audio_track, video_track) = get_test_tracks(false, false);

    peer.get_offer(vec![audio_track, video_track], None, false)
        .await
        .unwrap();

    let handle = room.new_handle();
    assert!(JsFuture::from(handle.mute_video()).await.is_ok());
    assert!(!peer.is_send_video_enabled());
    assert!(JsFuture::from(handle.unmute_video()).await.is_ok());
    assert!(peer.is_send_video_enabled());
}

/// Tests that two simultaneous calls of [`RoomHandle::mute_audio`] method will
/// be resolved normally.
///
/// # Algorithm
///
/// 1. Create [`Room`] in [`MuteState::NotMuted`].
///
/// 2. Call [`RoomHandle::mute_audio`] simultaneous twice.
///
/// 3. Check that [`PeerConnection`] with [`TransceiverKind::Audio`] of [`Room`]
///    is in [`MuteState::Muted`].
#[wasm_bindgen_test]
async fn join_two_audio_mutes() {
    let (room, peer) = get_test_room_and_exist_peer(6);
    let (audio_track, video_track) = get_test_tracks(false, false);

    peer.get_offer(vec![audio_track, video_track], None, false)
        .await
        .unwrap();

    let handle = room.new_handle();
    let (first, second) = futures::future::join(
        JsFuture::from(handle.mute_audio()),
        JsFuture::from(handle.mute_audio()),
    )
    .await;
    first.unwrap();
    second.unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Audio,
        StableMuteState::Muted
    ));
}

/// Tests that two simultaneous calls of [`RoomHandle::mute_video`] method will
/// both be resolved.
///
/// # Algorithm
///
/// 1. Create [`Room`] in [`MuteState::NotMuted`].
///
/// 2. Call [`RoomHandle::mute_video`] simultaneous twice.
///
/// 3. Check that [`PeerConnection`] with [`TransceiverKind::Video`] of [`Room`]
///    is in [`MuteState::Muted`].
#[wasm_bindgen_test]
async fn join_two_video_mutes() {
    let (room, peer) = get_test_room_and_exist_peer(6);
    let (audio_track, video_track) = get_test_tracks(false, false);

    peer.get_offer(vec![audio_track, video_track], None, false)
        .await
        .unwrap();

    let handle = room.new_handle();
    let (first, second) = futures::future::join(
        JsFuture::from(handle.mute_video()),
        JsFuture::from(handle.mute_video()),
    )
    .await;
    first.unwrap();
    second.unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Video,
        StableMuteState::Muted
    ));
}

/// Tests that if [`RoomHandle::mute_audio`] and [`RoomHandle::unmute_audio`]
/// are called simultaneously, then first call will be rejected, and second
/// resolved.
///
/// # Algorithm
///
/// 1. Create [`Room`] in [`MuteState::NotMuted`].
///
/// 2. Call [`RoomHandle::mute_audio`] and [`RoomHandle::unmute_audio`]
///    simultaneous.
///
/// 3. Check that [`PeerConnection`] with [`TransceiverKind::Audio`] of [`Room`]
///    is stayed in [`MuteState::NotMuted`].
#[wasm_bindgen_test]
async fn join_mute_and_unmute_audio() {
    let (room, peer) = get_test_room_and_exist_peer(5);
    let (audio_track, video_track) = get_test_tracks(false, false);

    peer.get_offer(vec![audio_track, video_track], None, false)
        .await
        .unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Audio,
        StableMuteState::NotMuted
    ));

    let handle = room.new_handle();
    let (mute_audio_result, unmute_audio_result) = futures::future::join(
        JsFuture::from(handle.mute_audio()),
        JsFuture::from(handle.unmute_audio()),
    )
    .await;
    mute_audio_result.unwrap_err();
    unmute_audio_result.unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Audio,
        StableMuteState::NotMuted
    ));
}

/// Tests that if [`RoomHandle::mute_video`] and [`RoomHandle::unmute_video`]
/// are called simultaneously, then first call will be rejected, and second
/// resolved.
///
/// # Algorithm
///
/// 1. Create [`Room`] in [`MuteState::NotMuted`].
///
/// 2. Call [`RoomHandle::mute_video`] and [`RoomHandle::unmute_video`]
///    simultaneous.
///
/// 3. Check that [`PeerConnection`] with [`TransceiverKind::Video`] of [`Room`]
///    is stayed in [`MuteState::NotMuted`].
#[wasm_bindgen_test]
async fn join_mute_and_unmute_video() {
    let (room, peer) = get_test_room_and_exist_peer(5);
    let (audio_track, video_track) = get_test_tracks(false, false);

    peer.get_offer(vec![audio_track, video_track], None, false)
        .await
        .unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Video,
        StableMuteState::NotMuted
    ));

    let handle = room.new_handle();
    let (mute_video_result, unmute_video_result) = futures::future::join(
        JsFuture::from(handle.mute_video()),
        JsFuture::from(handle.unmute_video()),
    )
    .await;
    mute_video_result.unwrap_err();
    unmute_video_result.unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Video,
        StableMuteState::NotMuted
    ));
}

/// Tests that simultaneous calls of [`RoomHandle::mute_video`] and
/// [`RoomHandle::unmute_video`] on [`Room`] with video in
/// [`MuteState::Muted`] not goes into an infinite loop.
///
/// # Algorithm
///
/// 1. Create [`Room`] video tracks in [`MuteState::Muted`].
///
/// 2. Call [`RoomHandle::mute_video`] and [`RoomHandle::unmute_video`]
///    simultaneous.
///
/// 3. Check that [`PeerConnection`] with [`TransceiverKind::Video`] of [`Room`]
///    is in [`MuteState::NotMuted`].
#[wasm_bindgen_test]
async fn join_unmute_and_mute_audio() {
    let (room, peer) = get_test_room_and_exist_peer(7);
    let (audio_track, video_track) = get_test_tracks(false, false);

    peer.get_offer(vec![audio_track, video_track], None, false)
        .await
        .unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Audio,
        StableMuteState::NotMuted
    ));

    let handle = room.new_handle();
    JsFuture::from(handle.mute_audio()).await.unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Audio,
        StableMuteState::Muted
    ));

    let (mute_audio_result, unmute_audio_result) = futures::future::join(
        JsFuture::from(handle.mute_audio()),
        JsFuture::from(handle.unmute_audio()),
    )
    .await;
    mute_audio_result.unwrap();
    unmute_audio_result.unwrap();

    assert!(peer.is_all_senders_in_mute_state(
        TransceiverKind::Audio,
        StableMuteState::NotMuted
    ));
}

fn get_test_room_and_new_peer(
    event_rx: mpsc::UnboundedReceiver<Event>,
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
            false,
        )
        .unwrap(),
    );
    let peer_clone = Rc::clone(&peer);
    repo.expect_create_peer()
        .withf(
            move |id: &PeerId,
                  _ice_servers: &Vec<IceServer>,
                  _peer_events_sender: &mpsc::UnboundedSender<PeerEvent>,
                  _is_force_relay: &bool| { *id == PeerId(1) },
        )
        .return_once_st(move |_, _, _, _| Ok(peer_clone));
    rpc.expect_send_command().return_const(());
    rpc.expect_unsub().return_const(());
    rpc.expect_set_close_reason().return_const(());

    let room = Room::new(Rc::new(rpc), repo);
    (room, peer)
}

// TODO: Allow muting before Peer init (instrumentisto/medea#85).
//
//#[wasm_bindgen_test]
// async fn mute_audio_room_before_init_peer() {
//    let (event_tx, event_rx) = mpsc::unbounded();
//    let (room, peer) = get_test_room_and_new_peer(event_rx);
//    let (audio_track, video_track) = get_test_tracks(true, true);
//
//    JsFuture::from(room.new_handle().mute_audio())
//        .await
//        .unwrap();
//    event_tx
//        .unbounded_send(Event::PeerCreated {
//            peer_id: PeerId(1),
//            sdp_offer: None,
//            tracks: vec![audio_track, video_track],
//            ice_servers: vec![],
//            force_relay: false,
//        })
//        .unwrap();
//
//    resolve_after(500).await.unwrap();
//
//    assert!(peer.is_send_video_enabled());
//    assert!(!peer.is_send_audio_enabled());
//}
//
//#[wasm_bindgen_test]
// async fn mute_video_room_before_init_peer() {
//    let (event_tx, event_rx) = mpsc::unbounded();
//    let (room, peer) = get_test_room_and_new_peer(event_rx);
//    let (audio_track, video_track) = get_test_tracks(true, true);
//
//    JsFuture::from(room.new_handle().mute_video())
//        .await
//        .unwrap();
//    event_tx
//        .unbounded_send(Event::PeerCreated {
//            peer_id: PeerId(1),
//            sdp_offer: None,
//            tracks: vec![audio_track, video_track],
//            ice_servers: vec![],
//            force_relay: false,
//        })
//        .unwrap();
//
//    resolve_after(500).await.unwrap();
//
//    assert!(peer.is_send_audio_enabled());
//    assert!(!peer.is_send_video_enabled());
//}

/// Tests RoomHandle::set_local_media_settings before creating PeerConnection.
/// Setup:
///     1. Create Room.
///     2. Set `on_failed_local_stream` callback.
///     3. Invoke `room_handle.set_local_media_settings` with one track.
///     4. Send `PeerCreated` to room wth two tracks
/// Assertions:
///     1. `on_failed_local_stream` callback was invoked.
#[wasm_bindgen_test]
async fn error_inject_invalid_local_stream_into_new_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, _peer) = get_test_room_and_new_peer(event_rx);

    let room_handle = room.new_handle();
    let (cb, test_result) = js_callback!(|err: JasonError| {
        cb_assert_eq!(&err.name(), "InvalidLocalStream");
        cb_assert_eq!(
            err.message(),
            "Invalid local stream: provided MediaStream was expected to have \
             single video track"
        );
    });
    room_handle.on_failed_local_stream(cb.into()).unwrap();

    let (audio_track, video_track) = get_test_tracks(false, false);

    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());

    JsFuture::from(room_handle.set_local_media_settings(&constraints))
        .await
        .unwrap();

    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            sdp_offer: None,
            tracks: vec![audio_track, video_track],
            ice_servers: vec![],
            force_relay: false,
        })
        .unwrap();

    wait_and_check_test_result(test_result, || {}).await;
}

/// Tests RoomHandle::set_local_media_settings for existing PeerConnection.
/// Setup:
///     1. Create Room.
///     2. Set `on_failed_local_stream` callback.
///     3. Invoke `peer.get_offer` with two tracks.
///     4. Invoke `room_handle.set_local_media_settings` with only one track.
/// Assertions:
///     1. `on_failed_local_stream` was invoked.
#[wasm_bindgen_test]
async fn error_inject_invalid_local_stream_into_room_on_exists_peer() {
    let (cb, test_result) = js_callback!(|err: JasonError| {
        cb_assert_eq!(&err.name(), "InvalidLocalStream");
        cb_assert_eq!(
            &err.message(),
            "Invalid local stream: provided MediaStream was expected to have \
             single video track"
        );
    });
    let (room, peer) = get_test_room_and_exist_peer(1);
    let (audio_track, video_track) = get_test_tracks(false, false);
    peer.get_offer(vec![audio_track, video_track], None, false)
        .await
        .unwrap();

    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());
    let room_handle = room.new_handle();
    room_handle.on_failed_local_stream(cb.into()).unwrap();
    JsFuture::from(room_handle.set_local_media_settings(&constraints))
        .await
        .unwrap();

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn error_get_local_stream_on_new_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, _peer) = get_test_room_and_new_peer(event_rx);

    let room_handle = room.new_handle();

    let (cb, test_result) = js_callback!(|err: JasonError| {
        cb_assert_eq!(&err.name(), "CouldNotGetLocalMedia");
        cb_assert_eq!(
            &err.message(),
            "Failed to get local stream: MediaDevices.getUserMedia() failed: \
             Unknown JS error: error_get_local_stream_on_new_peer"
        );
    });

    room_handle.on_failed_local_stream(cb.into()).unwrap();

    let mock_navigator = MockNavigator::new();
    mock_navigator
        .error_get_user_media("error_get_local_stream_on_new_peer".into());

    let (audio_track, video_track) = get_test_tracks(false, false);
    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            sdp_offer: None,
            tracks: vec![audio_track, video_track],
            ice_servers: vec![],
            force_relay: false,
        })
        .unwrap();

    wait_and_check_test_result(test_result, move || mock_navigator.stop())
        .await;
}

/// Tests `Room::join` if `on_failed_local_stream` callback was not set.
/// Setup:
///     1. Create Room.
///     2. DO NOT set `on_failed_local_stream` callback.
///     3. Try join to Room.
/// Assertions:
///     1. Room::join returns error.
#[wasm_bindgen_test]
async fn error_join_room_without_on_failed_stream_callback() {
    let (_, event_rx) = mpsc::unbounded();
    let mut rpc = MockRpcClient::new();
    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    rpc.expect_unsub().return_const(());
    rpc.expect_set_close_reason().return_const(());
    let repo = Box::new(MockPeerRepository::new());
    let room = Room::new(Rc::new(rpc), repo);

    let room_handle = room.new_handle();
    room_handle
        .on_connection_loss(js_sys::Function::new_no_args(""))
        .unwrap();

    match room_handle.inner_join(String::from("token")).await {
        Ok(_) => unreachable!(),
        Err(e) => {
            assert_eq!(e.name(), "CallbackNotSet");
            assert_eq!(
                e.message(),
                "`Room.on_failed_local_stream()` callback isn't set.",
            );
            assert!(!e.trace().is_empty());
        }
    }
}

/// Tests `Room::join` if `on_connection_loss` callback was not set.
/// Setup:
///     1. Create Room.
///     2. DO NOT set `on_connection_loss` callback.
///     3. Try join to Room.
/// Assertions:
///     1. Room::join returns error.
#[wasm_bindgen_test]
async fn error_join_room_without_on_connection_loss_callback() {
    let (_, event_rx) = mpsc::unbounded();
    let mut rpc = MockRpcClient::new();
    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    rpc.expect_unsub().return_const(());
    rpc.expect_set_close_reason().return_const(());
    let repo = Box::new(MockPeerRepository::new());
    let room = Room::new(Rc::new(rpc), repo);

    let room_handle = room.new_handle();
    room_handle
        .on_failed_local_stream(js_sys::Function::new_no_args(""))
        .unwrap();

    match room_handle.inner_join(String::from("token")).await {
        Ok(_) => unreachable!(),
        Err(e) => {
            assert_eq!(e.name(), "CallbackNotSet");
            assert_eq!(
                e.message(),
                "`Room.on_connection_loss()` callback isn't set.",
            );
            assert!(!e.trace().is_empty());
        }
    }
}

/// Tests for `RoomHandle.on_close` JS side callback.
mod on_close_callback {
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
                                closed.reason(); }")]
    extern "C" {
        fn get_reason(closed: &JsValue) -> String;
    }
    #[wasm_bindgen(inline_js = "export function \
                                get_is_closed_by_server(reason) { return \
                                reason.is_closed_by_server(); }")]
    extern "C" {
        fn get_is_closed_by_server(reason: &JsValue) -> bool;
    }
    #[wasm_bindgen(inline_js = "export function get_is_err(reason) { return \
                                reason.is_err(); }")]
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
        wait_and_check_test_result(test_result, || {}).await;
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

        drop(room);
        wait_and_check_test_result(test_result, || {}).await;
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
        wait_and_check_test_result(test_result, || {}).await;
    }
}

mod rpc_close_reason_on_room_drop {
    //! Tests which checks that when [`Room`] is dropped, the right close reason
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

        drop(room);

        let close_reason = test_rx.await.unwrap();
        assert_eq!(
            close_reason,
            ClientDisconnect::RoomUnexpectedlyDropped,
            "Room sets RPC close reason '{:?} instead of \
             'RoomUnexpectedlyDropped'.",
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
            close_reason,
            ClientDisconnect::RoomClosed,
            "Room sets RPC close reason '{:?}' instead of 'RoomClosed.",
            close_reason,
        );
    }
}

/// Tests for [`TrackPatch`] generation in [`Room`].
mod patches_generation {
    use std::collections::HashMap;

    use futures::StreamExt;
    use medea_client_api_proto::{
        AudioSettings, Direction, MediaType, Track, TrackId, TrackPatch,
        VideoSettings,
    };
    use wasm_bindgen_futures::spawn_local;

    use crate::await_with_timeout;

    use super::*;

    /// Returns [`Room`] with mocked [`PeerRepository`] with provided count of
    /// [`PeerConnection`]s and [`mpsc::UnboundedReceiver`] of [`Command`]s
    /// sent from this [`Room`].
    ///
    /// `audio_track_muted_state_fn`'s output will be used as `is_muted` value
    /// for all audio [`Track`]s.
    async fn get_room_and_commands_receiver(
        peers_count: u64,
        audio_track_muted_state_fn: impl Fn(u64) -> bool,
    ) -> (Room, mpsc::UnboundedReceiver<Command>) {
        let mut repo = Box::new(MockPeerRepository::new());

        let mut peers = HashMap::new();
        for i in 0..peers_count {
            let (tx, _rx) = mpsc::unbounded();
            let audio_track_id = TrackId(i + 1);
            let video_track_id = TrackId(i + 2);
            let audio_track = Track {
                id: audio_track_id,
                is_muted: (audio_track_muted_state_fn)(i),
                media_type: MediaType::Audio(AudioSettings {}),
                direction: Direction::Send {
                    receivers: Vec::new(),
                    mid: None,
                },
            };
            let video_track = Track {
                id: video_track_id,
                is_muted: false,
                media_type: MediaType::Video(VideoSettings {}),
                direction: Direction::Send {
                    receivers: Vec::new(),
                    mid: None,
                },
            };
            let tracks = vec![audio_track, video_track];
            let peer_id = PeerId(i + 1);

            let peer = Rc::new(
                PeerConnection::new(
                    peer_id,
                    tx,
                    vec![],
                    Rc::new(MediaManager::default()),
                    false,
                )
                .unwrap(),
            );

            peer.get_offer(tracks, None, false).await.unwrap();

            peers.insert(peer_id, peer);
        }

        let repo_get_all: Vec<_> =
            peers.iter().map(|(_, peer)| Rc::clone(peer)).collect();
        repo.expect_get_all()
            .returning_st(move || repo_get_all.clone());
        repo.expect_get()
            .returning_st(move |id| peers.get(&id).cloned());

        let mut rpc = MockRpcClient::new();
        let (command_tx, command_rx) = mpsc::unbounded();
        rpc.expect_send_command().returning(move |command| {
            command_tx.unbounded_send(command).unwrap();
        });
        rpc.expect_subscribe()
            .return_once(move || Box::pin(futures::stream::pending()));
        rpc.expect_unsub().return_once(|| ());
        rpc.expect_set_close_reason().return_once(|_| ());

        (Room::new(Rc::new(rpc), repo), command_rx)
    }

    /// Tests that [`Room`] normally generates [`TrackPatch`]s when have one
    /// [`PeerConnection`] with one unmuted video [`Track`] and one unmuted
    /// audio [`Track`].
    ///
    /// # Algorithm
    ///
    /// 1. Get mock of [`Room`] and [`Command`]s receiver of this [`Room`] with
    ///    one [`PeerConnection`]s.
    ///
    /// 2. Call [`RoomHandle::mute_audio`].
    ///
    /// 3. Check that [`Room`] tries to send one [`Command::UpdateTracks`] with
    ///    one [`TrackPatch`] for audio [`Track`].
    #[wasm_bindgen_test]
    async fn track_patch_for_all_video() {
        let (room, mut command_rx) =
            get_room_and_commands_receiver(1, |_| false).await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.mute_audio()).await.unwrap();
        });

        assert_eq!(
            command_rx.next().await.unwrap(),
            Command::UpdateTracks {
                peer_id: PeerId(1),
                tracks_patches: vec![TrackPatch {
                    id: TrackId(1),
                    is_muted: Some(true),
                }]
            }
        );
    }

    /// Tests that [`Room`] normally generates [`TrackPatch`]s when have two
    /// [`PeerConnection`] with one unmuted video [`Track`] and one unmuted
    /// audio [`Track`] in both [`PeerConnection`]s.
    ///
    /// # Algorithm
    ///
    /// 1. Get mock of [`Room`] and [`Command`]s receiver of this [`Room`] with
    ///    two [`PeerConnection`]s.
    ///
    /// 2. Call [`RoomHandle::mute_audio`].
    ///
    /// 3. Check that [`Room`] tries to send two [`Command::UpdateTracks`] for
    ///    unmuted [`PeerConnection`]s. [`PeerConnection`]s.
    #[wasm_bindgen_test]
    async fn track_patch_for_many_tracks() {
        let (room, mut command_rx) =
            get_room_and_commands_receiver(2, |_| false).await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.mute_audio()).await.unwrap();
        });

        let mut commands = HashMap::new();
        for _ in 0..2i32 {
            let command = command_rx.next().await.unwrap();
            match command {
                Command::UpdateTracks {
                    peer_id,
                    tracks_patches,
                } => {
                    commands.insert(peer_id, tracks_patches);
                }
                _ => (),
            }
        }

        assert_eq!(
            commands.remove(&PeerId(1)).unwrap(),
            vec![TrackPatch {
                id: TrackId(1),
                is_muted: Some(true),
            }]
        );

        assert_eq!(
            commands.remove(&PeerId(2)).unwrap(),
            vec![TrackPatch {
                id: TrackId(2),
                is_muted: Some(true),
            }]
        );
    }

    /// Tests that [`Room`] wouldn't generate [`TrackPatch`]s for already
    /// unmuted [`PeerConnection`]s.
    ///
    /// # Algorithm
    ///
    /// 1. Get mock of [`Room`] and [`Command`]s receiver of this [`Room`] with
    ///    two [`PeerConnection`]s.
    ///
    /// 2. Call [`RoomHandle::unmute_audio`].
    ///
    /// 3. Check that [`Room`] doesn't send [`Command::UpdateTracks`] with
    ///    [`RpcClient`].
    #[wasm_bindgen_test]
    async fn try_to_unmute_unmuted() {
        let (room, mut command_rx) =
            get_room_and_commands_receiver(2, |_| false).await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.unmute_audio()).await.unwrap();
        });

        assert!(await_with_timeout(Box::pin(command_rx.next()), 5)
            .await
            .is_err());
    }

    /// Tests that [`Room`] will generate [`Command::UpdateTracks`] only for
    /// unmuted [`PeerConnection`].
    ///
    /// # Algorithm
    ///
    /// 1. Get mock of [`Room`] and [`Command`]s receiver of this [`Room`] with
    ///    one unmuted [`PeerConnection`]s and one muted [`PeerConnection`].
    ///
    /// 2. Call [`RoomHandle::mute_audio`].
    ///
    /// 3. Check that [`Room`] tries to send [`Command::UpdateTracks`] only for
    ///    unmuted [`PeerConnection`].
    #[wasm_bindgen_test]
    async fn mute_room_with_one_muted_track() {
        let (room, mut command_rx) =
            get_room_and_commands_receiver(2, |i| i % 2 == 0).await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.mute_audio()).await.unwrap();
        });

        assert_eq!(
            command_rx.next().await.unwrap(),
            Command::UpdateTracks {
                peer_id: PeerId(2),
                tracks_patches: vec![TrackPatch {
                    id: TrackId(2),
                    is_muted: Some(true),
                }]
            }
        );
    }
}
