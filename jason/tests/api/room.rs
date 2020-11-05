#![cfg(target_arch = "wasm32")]

use std::{collections::HashMap, rc::Rc};

use futures::{
    channel::{
        mpsc::{self, UnboundedReceiver},
        oneshot,
    },
    stream::{self, BoxStream, StreamExt as _},
};
use medea_client_api_proto::{
    Command, Event, NegotiationRole, PeerId, Track, TrackId, TrackUpdate,
};
use medea_jason::{
    api::Room,
    media::{
        AudioTrackConstraints, MediaKind, MediaManager, MediaStreamSettings,
    },
    peer::{MockPeerRepository, PeerConnection, Repository},
    rpc::MockRpcClient,
    utils::JasonError,
    DeviceVideoTrackConstraints,
};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use wasm_bindgen_test::*;

use crate::{
    delay_for, get_test_recv_tracks, get_test_required_tracks, get_test_tracks,
    get_test_unrequired_tracks, media_stream_settings, timeout,
    wait_and_check_test_result, yield_now, MockNavigator,
};

wasm_bindgen_test_configure!(run_in_browser);

fn get_test_room(
    events: BoxStream<'static, Event>,
) -> (Room, UnboundedReceiver<Command>) {
    let (tx, rx) = mpsc::unbounded();
    let mut rpc = MockRpcClient::new();

    rpc.expect_subscribe().return_once(move || events);
    rpc.expect_unsub().return_const(());
    rpc.expect_set_close_reason().return_const(());
    rpc.expect_on_connection_loss()
        .return_once(|| stream::pending().boxed_local());
    rpc.expect_on_reconnected()
        .return_once(|| stream::pending().boxed_local());
    rpc.expect_send_command().returning(move |command| {
        let _ = tx.unbounded_send(command);
    });

    (
        Room::new(Rc::new(rpc), Box::new(Repository::new(Rc::default()))),
        rx,
    )
}

async fn get_test_room_and_exist_peer(
    tracks: Vec<Track>,
    media_stream_settings: Option<MediaStreamSettings>,
) -> (Room, Rc<PeerConnection>) {
    let mut rpc = MockRpcClient::new();

    let (event_tx, event_rx) = mpsc::unbounded();

    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    rpc.expect_unsub().return_const(());
    rpc.expect_on_connection_loss()
        .return_once(|| stream::pending().boxed_local());
    rpc.expect_on_reconnected()
        .return_once(|| stream::pending().boxed_local());
    rpc.expect_set_close_reason().return_const(());
    let event_tx_clone = event_tx.clone();
    rpc.expect_send_command().returning(move |cmd| match cmd {
        Command::UpdateTracks {
            peer_id,
            tracks_patches,
        } => {
            event_tx_clone
                .unbounded_send(Event::TracksApplied {
                    peer_id,
                    updates: tracks_patches
                        .into_iter()
                        .map(|p| TrackUpdate::Updated(p.into()))
                        .collect(),
                    negotiation_role: None,
                })
                .unwrap();
        }
        _ => (),
    });

    let room =
        Room::new(Rc::new(rpc), Box::new(Repository::new(Rc::default())));
    if let Some(media_stream_settings) = &media_stream_settings {
        JsFuture::from(
            room.new_handle()
                .set_local_media_settings(&media_stream_settings),
        )
        .await
        .unwrap();
    }
    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            negotiation_role: NegotiationRole::Offerer,
            tracks,
            ice_servers: Vec::new(),
            force_relay: false,
        })
        .unwrap();

    // wait until Event::PeerCreated is handled
    delay_for(200).await;
    let peer = room.get_peer_by_id(PeerId(1)).unwrap();
    (room, peer)
}

/// Tests RoomHandle::set_local_media_settings before creating PeerConnection.
/// Setup:
///     1. Create Room.
///     2. Set `on_failed_local_media` callback.
///     3. Invoke `room_handle.set_local_media_settings` with one track.
///     4. Send `PeerCreated` to room wth two tracks
/// Assertions:
///     1. `on_failed_local_media` callback was invoked.
#[wasm_bindgen_test]
async fn error_inject_invalid_local_stream_into_new_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, _rx) = get_test_room(Box::pin(event_rx));
    let room_handle = room.new_handle();

    let (cb, test_result) = js_callback!(|err: JasonError| {
        cb_assert_eq!(&err.name(), "InvalidLocalTracks");
        cb_assert_eq!(
            err.message(),
            "Invalid local tracks: MediaExchangeState of Sender can\'t be \
             transited into disabled state, because this Sender is required."
        );
    });
    room_handle.on_failed_local_media(cb.into()).unwrap();

    let (audio_track, video_track) = get_test_required_tracks();

    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());

    JsFuture::from(room_handle.set_local_media_settings(&constraints))
        .await
        .unwrap();

    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            negotiation_role: NegotiationRole::Offerer,
            tracks: vec![audio_track, video_track],
            ice_servers: Vec::new(),
            force_relay: false,
        })
        .unwrap();

    wait_and_check_test_result(test_result, || {}).await;
}

/// Tests RoomHandle::set_local_media_settings for existing PeerConnection.
/// Setup:
///     1. Create Room.
///     2. Set `on_failed_local_media` callback.
///     3. Invoke `peer.get_offer` with two tracks.
///     4. Invoke `room_handle.set_local_media_settings` with only one track.
/// Assertions:
///     1. `on_failed_local_media` was invoked.
#[wasm_bindgen_test]
async fn error_inject_invalid_local_stream_into_room_on_exists_peer() {
    let (cb, test_result) = js_callback!(|err: JasonError| {
        cb_assert_eq!(&err.name(), "InvalidLocalTracks");
        cb_assert_eq!(
            &err.message(),
            "Invalid local tracks: provided multiple device video \
             MediaStreamTracks"
        );
    });
    let (audio_track, video_track) = get_test_required_tracks();
    let (room, _peer) =
        get_test_room_and_exist_peer(vec![audio_track, video_track], None)
            .await;

    let mut constraints = MediaStreamSettings::new();
    constraints.audio(AudioTrackConstraints::new());
    let room_handle = room.new_handle();
    room_handle.on_failed_local_media(cb.into()).unwrap();
    JsFuture::from(room_handle.set_local_media_settings(&constraints))
        .await
        .unwrap();

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn no_errors_if_track_not_provided_when_its_optional() {
    async fn helper(
        audio_required: bool,
        video_required: bool,
        add_audio: bool,
        add_video: bool,
    ) -> Result<(), ()> {
        let (test_tx, test_rx) = oneshot::channel();
        let closure = wasm_bindgen::closure::Closure::once_into_js(move || {
            test_tx.send(()).unwrap();
        });
        let (audio_track, video_track) =
            get_test_tracks(audio_required, video_required);
        let (room, _peer) =
            get_test_room_and_exist_peer(vec![audio_track, video_track], None)
                .await;

        let mut constraints = MediaStreamSettings::new();
        if add_audio {
            constraints.audio(AudioTrackConstraints::new());
        }
        if add_video {
            constraints.device_video(DeviceVideoTrackConstraints::new());
        }

        let room_handle = room.new_handle();
        room_handle.on_failed_local_media(closure.into()).unwrap();
        JsFuture::from(room_handle.set_local_media_settings(&constraints))
            .await
            .unwrap();

        timeout(1000, test_rx)
            .await
            .map(|rx| rx.unwrap())
            .map_err(|_| ())
    }

    // on_failed_local_media callback does not fire
    helper(true, false, true, false).await.unwrap_err();
    helper(false, true, false, true).await.unwrap_err();
    helper(false, false, false, false).await.unwrap_err();

    // on_failed_local_media callback fires
    helper(true, false, false, true).await.unwrap();
    helper(false, true, true, false).await.unwrap();
    helper(true, true, false, false).await.unwrap();
}

#[wasm_bindgen_test]
async fn error_get_local_stream_on_new_peer() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, _) = get_test_room(Box::pin(event_rx));
    let room_handle = room.new_handle();
    JsFuture::from(
        room_handle
            .set_local_media_settings(&media_stream_settings(true, true)),
    )
    .await
    .unwrap();

    let (cb, test_result) = js_callback!(|err: JasonError| {
        cb_assert_eq!(&err.name(), "CouldNotGetLocalMedia");
        cb_assert_eq!(
            &err.message(),
            "Failed to get local tracks: MediaDevices.getUserMedia() failed: \
             Unknown JS error: error_get_local_stream_on_new_peer"
        );
    });

    room_handle.on_failed_local_media(cb.into()).unwrap();

    let mock_navigator = MockNavigator::new();
    mock_navigator
        .error_get_user_media("error_get_local_stream_on_new_peer".into());

    let (audio_track, video_track) = get_test_unrequired_tracks();
    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            negotiation_role: NegotiationRole::Offerer,
            tracks: vec![audio_track, video_track],
            ice_servers: Vec::new(),
            force_relay: false,
        })
        .unwrap();

    wait_and_check_test_result(test_result, move || mock_navigator.stop())
        .await;
}

/// Tests `Room::join` if `on_failed_local_media` callback was not set.
/// Setup:
///     1. Create Room.
///     2. DO NOT set `on_failed_local_media` callback.
///     3. Try join to Room.
/// Assertions:
///     1. Room::join returns error.
#[wasm_bindgen_test]
async fn error_join_room_without_on_failed_stream_callback() {
    let (room, _) = get_test_room(stream::pending().boxed());
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
                "`Room.on_failed_local_media()` callback isn't set.",
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
    let (room, _) = get_test_room(stream::pending().boxed());
    let room_handle = room.new_handle();

    room_handle
        .on_failed_local_media(js_sys::Function::new_no_args(""))
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

mod disable_recv_tracks {
    use medea_client_api_proto::{
        AudioSettings, Direction, MediaSourceKind, MediaType, MemberId,
        VideoSettings,
    };

    use super::*;

    #[wasm_bindgen_test]
    async fn check_transceivers_statuses() {
        let (event_tx, event_rx) = mpsc::unbounded();
        let (room, mut commands_rx) = get_test_room(Box::pin(event_rx));
        let room_handle = room.new_handle();

        JsFuture::from(room_handle.disable_remote_audio())
            .await
            .unwrap();

        event_tx
            .unbounded_send(Event::PeerCreated {
                peer_id: PeerId(1),
                negotiation_role: NegotiationRole::Offerer,
                tracks: vec![
                    Track {
                        id: TrackId(1),
                        direction: Direction::Send {
                            receivers: vec![MemberId::from("bob")],
                            mid: None,
                        },
                        media_type: MediaType::Audio(AudioSettings {
                            is_required: true,
                        }),
                    },
                    Track {
                        id: TrackId(2),
                        direction: Direction::Recv {
                            sender: MemberId::from("bob"),
                            mid: None,
                        },
                        media_type: MediaType::Video(VideoSettings {
                            is_required: true,
                            source_kind: MediaSourceKind::Device,
                        }),
                    },
                    Track {
                        id: TrackId(3),
                        direction: Direction::Recv {
                            sender: MemberId::from("bob"),
                            mid: None,
                        },
                        media_type: MediaType::Audio(AudioSettings {
                            is_required: true,
                        }),
                    },
                ],
                ice_servers: Vec::new(),
                force_relay: false,
            })
            .unwrap();

        delay_for(200).await;
        match commands_rx.next().await.unwrap() {
            Command::MakeSdpOffer {
                peer_id,
                sdp_offer: _,
                mids,
                transceivers_statuses,
            } => {
                assert_eq!(peer_id, PeerId(1));
                assert_eq!(mids.len(), 3);
                let audio_send =
                    transceivers_statuses.get(&TrackId(1)).unwrap();
                let video_recv =
                    transceivers_statuses.get(&TrackId(2)).unwrap();
                let audio_recv =
                    transceivers_statuses.get(&TrackId(3)).unwrap();

                assert!(audio_send); // not disable
                assert!(video_recv); // not disable
                assert!(!audio_recv); // disable
            }
            _ => unreachable!(),
        }

        // TODO: add is_recv_audio/video asserts
    }
}

/// Tests disabling tracks publishing.
mod disable_send_tracks {
    use medea_client_api_proto::{
        AudioSettings, Direction, MediaSourceKind, MediaType, MemberId,
        VideoSettings,
    };
    use medea_jason::{
        media::{JsMediaSourceKind, MediaKind},
        peer::{StableMediaExchangeState, TrackDirection},
    };

    use super::*;

    #[wasm_bindgen_test]
    async fn disable_enable_audio() {
        let (audio_track, video_track) = get_test_unrequired_tracks();
        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        let handle = room.new_handle();
        assert!(JsFuture::from(handle.disable_audio()).await.is_ok());
        assert!(!peer.is_send_audio_enabled());
        assert!(JsFuture::from(handle.enable_audio()).await.is_ok());
        assert!(peer.is_send_audio_enabled());
    }

    #[wasm_bindgen_test]
    async fn disable_enable_video() {
        let (audio_track, video_track) = get_test_unrequired_tracks();
        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        let handle = room.new_handle();
        assert!(JsFuture::from(handle.disable_video(None)).await.is_ok());
        assert!(!peer.is_send_video_enabled(None));
        assert!(JsFuture::from(handle.enable_video(None)).await.is_ok());
        assert!(peer.is_send_video_enabled(None));
    }

    fn audio_track(track_id: TrackId, is_required: bool) -> Track {
        Track {
            id: track_id,
            direction: Direction::Send {
                receivers: vec![MemberId::from("bob")],
                mid: None,
            },
            media_type: MediaType::Audio(AudioSettings { is_required }),
        }
    }

    fn video_track(
        track_id: TrackId,
        is_required: bool,
        source_kind: MediaSourceKind,
    ) -> Track {
        Track {
            id: track_id,
            direction: Direction::Send {
                receivers: vec![MemberId::from("bob")],
                mid: None,
            },
            media_type: MediaType::Video(VideoSettings {
                is_required,
                source_kind,
            }),
        }
    }

    /// Tests that when [`JsMediaSouceKind::Device`] is provided to the
    /// [`RoomHandle::disable_video`] and [`RoomHandle::enable_video`], the
    /// only device video will be disabled/enabled.
    #[wasm_bindgen_test]
    async fn disable_enable_device_video() {
        let audio_track = audio_track(TrackId(1), false);
        let device_video_track =
            video_track(TrackId(2), false, MediaSourceKind::Device);
        let display_video_track =
            video_track(TrackId(3), false, MediaSourceKind::Display);

        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, device_video_track, display_video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        let handle = room.new_handle();
        assert!(JsFuture::from(
            handle.disable_video(Some(JsMediaSourceKind::Device))
        )
        .await
        .is_ok());
        assert!(!peer.is_send_video_enabled(Some(MediaSourceKind::Device)));
        assert!(peer.is_send_video_enabled(Some(MediaSourceKind::Display)));
        assert!(JsFuture::from(
            handle.enable_video(Some(JsMediaSourceKind::Device))
        )
        .await
        .is_ok());
        assert!(peer.is_send_video_enabled(Some(MediaSourceKind::Device)));
        assert!(peer.is_send_video_enabled(Some(MediaSourceKind::Display)));
    }

    /// Tests that when [`JsMediaSouceKind::Display`] is provided to the
    /// [`RoomHandle::disable_video`] and [`RoomHandle::enable_video`], the
    /// only display video will be disabled/enabled.
    #[wasm_bindgen_test]
    async fn disable_enable_display_video() {
        let audio_track = audio_track(TrackId(1), false);
        let device_video_track =
            video_track(TrackId(2), false, MediaSourceKind::Device);
        let display_video_track =
            video_track(TrackId(3), false, MediaSourceKind::Display);

        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, device_video_track, display_video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        let handle = room.new_handle();
        assert!(JsFuture::from(
            handle.disable_video(Some(JsMediaSourceKind::Display))
        )
        .await
        .is_ok());
        assert!(!peer.is_send_video_enabled(Some(MediaSourceKind::Display)));
        assert!(peer.is_send_video_enabled(Some(MediaSourceKind::Device)));
        assert!(JsFuture::from(
            handle.enable_video(Some(JsMediaSourceKind::Display))
        )
        .await
        .is_ok());
        assert!(peer.is_send_video_enabled(Some(MediaSourceKind::Display)));
        assert!(peer.is_send_video_enabled(Some(MediaSourceKind::Device)));
    }

    /// Tests that two simultaneous calls of [`RoomHandle::disable_audio`]
    /// method will be resolved normally.
    ///
    /// # Algorithm
    ///
    /// 1. Create [`Room`] in [`MediaExchangeState::Enabled`].
    ///
    /// 2. Call [`RoomHandle::disable_audio`] simultaneous twice.
    ///
    /// 3. Check that [`PeerConnection`] with [`MediaKind::Audio`] of
    /// [`Room`] is in [`MediaExchangeState::Disabled`].
    #[wasm_bindgen_test]
    async fn join_two_audio_disables() {
        let (audio_track, video_track) = get_test_unrequired_tracks();
        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        let handle = room.new_handle();
        let (first, second) = futures::future::join(
            JsFuture::from(handle.disable_audio()),
            JsFuture::from(handle.disable_audio()),
        )
        .await;
        first.unwrap();
        second.unwrap();

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Audio,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Disabled.into()
        ));
    }

    /// Tests that two simultaneous calls of [`RoomHandle::disable_video`]
    /// method will both be resolved.
    ///
    /// # Algorithm
    ///
    /// 1. Create [`Room`] in [`MediaExchangeState::Enabled`].
    ///
    /// 2. Call [`RoomHandle::disable_video`] simultaneous twice.
    ///
    /// 3. Check that [`PeerConnection`] with [`MediaKind::Video`] of
    /// [`Room`] is in [`MediaExchangeState::Disabled`].
    #[wasm_bindgen_test]
    async fn join_two_video_disables() {
        let (audio_track, video_track) = get_test_unrequired_tracks();
        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        let handle = room.new_handle();
        let (first, second) = futures::future::join(
            JsFuture::from(handle.disable_video(None)),
            JsFuture::from(handle.disable_video(None)),
        )
        .await;
        first.unwrap();
        second.unwrap();

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Video,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Disabled.into()
        ));
    }

    /// Tests that if [`RoomHandle::disable_audio`] and
    /// [`RoomHandle::enable_audio`] are called simultaneously, then first
    /// call will be rejected, and second resolved.
    ///
    /// # Algorithm
    ///
    /// 1. Create [`Room`] in [`MediaExchangeState::Enabled`].
    ///
    /// 2. Call [`RoomHandle::disable_audio`] and [`RoomHandle::enable_audio`]
    ///    simultaneous.
    ///
    /// 3. Check that [`PeerConnection`] with [`MediaKind::Audio`] of
    /// [`Room`] is stayed in [`MediaExchangeState::Enabled`].
    #[wasm_bindgen_test]
    async fn join_disable_and_enable_audio() {
        let (audio_track, video_track) = get_test_unrequired_tracks();
        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Audio,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Enabled.into()
        ));

        let handle = room.new_handle();
        let (disable_audio_result, enable_audio_result) =
            futures::future::join(
                JsFuture::from(handle.disable_audio()),
                JsFuture::from(handle.enable_audio()),
            )
            .await;
        disable_audio_result.unwrap_err();
        enable_audio_result.unwrap();

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Audio,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Enabled.into()
        ));
    }

    /// Tests that if [`RoomHandle::disable_video`] and
    /// [`RoomHandle::enable_video`] are called simultaneously, then first
    /// call will be rejected, and second resolved.
    ///
    /// # Algorithm
    ///
    /// 1. Create [`Room`] in [`MediaExchangeState::Enabled`].
    ///
    /// 2. Call [`RoomHandle::disable_video`] and [`RoomHandle::enable_video`]
    ///    simultaneous.
    ///
    /// 3. Check that [`PeerConnection`] with [`MediaKind::Video`] of
    /// [`Room`] is stayed in [`MediaExchangeState::Enabled`].
    #[wasm_bindgen_test]
    async fn join_disable_and_enable_video() {
        let (audio_track, video_track) = get_test_unrequired_tracks();
        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Video,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Enabled.into()
        ));

        let handle = room.new_handle();
        let (disable_video_result, enable_video_result) =
            futures::future::join(
                JsFuture::from(handle.disable_video(None)),
                JsFuture::from(handle.enable_video(None)),
            )
            .await;
        disable_video_result.unwrap_err();
        enable_video_result.unwrap();

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Video,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Enabled.into()
        ));
    }

    /// Tests that simultaneous calls of [`RoomHandle::disable_video`] and
    /// [`RoomHandle::enable_video`] on [`Room`] with video in
    /// [`MediaExchangeState::Disabled`] not goes into an infinite loop.
    ///
    /// # Algorithm
    ///
    /// 1. Create [`Room`] video tracks in [`MediaExchangeState::Disabled`].
    ///
    /// 2. Call [`RoomHandle::disable_video`] and [`RoomHandle::enable_video`]
    ///    simultaneous.
    ///
    /// 3. Check that [`PeerConnection`] with [`MediaKind::Video`] of
    /// [`Room`] is in [`MediaExchangeState::Enabled`].
    #[wasm_bindgen_test]
    async fn join_enable_and_disable_audio() {
        let (audio_track, video_track) = get_test_unrequired_tracks();
        let (room, peer) = get_test_room_and_exist_peer(
            vec![audio_track, video_track],
            Some(media_stream_settings(true, true)),
        )
        .await;

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Audio,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Enabled.into()
        ));

        let handle = room.new_handle();
        JsFuture::from(handle.disable_audio()).await.unwrap();

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Audio,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Disabled.into()
        ));

        let (disable_audio_result, enable_audio_result) =
            futures::future::join(
                JsFuture::from(handle.disable_audio()),
                JsFuture::from(handle.enable_audio()),
            )
            .await;
        disable_audio_result.unwrap();
        enable_audio_result.unwrap();

        assert!(peer.is_all_transceiver_sides_in_media_state(
            MediaKind::Audio,
            TrackDirection::Send,
            None,
            StableMediaExchangeState::Enabled.into()
        ));
    }

    #[wasm_bindgen_test]
    async fn disable_audio_room_before_init_peer() {
        let (event_tx, event_rx) = mpsc::unbounded();
        let (room, mut commands_rx) = get_test_room(Box::pin(event_rx));
        JsFuture::from(
            room.new_handle()
                .set_local_media_settings(&media_stream_settings(true, true)),
        )
        .await
        .unwrap();

        JsFuture::from(room.new_handle().disable_audio())
            .await
            .unwrap();

        let (audio_track, video_track) = get_test_tracks(false, false);
        event_tx
            .unbounded_send(Event::PeerCreated {
                peer_id: PeerId(1),
                negotiation_role: NegotiationRole::Offerer,
                tracks: vec![audio_track, video_track],
                ice_servers: Vec::new(),
                force_relay: false,
            })
            .unwrap();

        delay_for(200).await;
        match commands_rx.next().await.unwrap() {
            Command::MakeSdpOffer {
                peer_id,
                sdp_offer: _,
                mids,
                transceivers_statuses,
            } => {
                assert_eq!(peer_id, PeerId(1));
                assert_eq!(mids.len(), 2);
                let audio = transceivers_statuses.get(&TrackId(1)).unwrap();
                let video = transceivers_statuses.get(&TrackId(2)).unwrap();

                assert!(!audio); // disabled
                assert!(video); // not disabled
            }
            _ => unreachable!(),
        }

        let peer = room.get_peer_by_id(PeerId(1)).unwrap();
        assert!(peer.is_send_video_enabled(None));
        assert!(!peer.is_send_audio_enabled());
    }

    #[wasm_bindgen_test]
    async fn enable_video_room_before_init_peer() {
        let (event_tx, event_rx) = mpsc::unbounded();
        let (room, mut commands_rx) = get_test_room(Box::pin(event_rx));
        JsFuture::from(
            room.new_handle()
                .set_local_media_settings(&media_stream_settings(true, true)),
        )
        .await
        .unwrap();

        JsFuture::from(room.new_handle().disable_video(None))
            .await
            .unwrap();

        let (audio_track, video_track) = get_test_tracks(false, false);
        event_tx
            .unbounded_send(Event::PeerCreated {
                peer_id: PeerId(1),
                negotiation_role: NegotiationRole::Offerer,
                tracks: vec![audio_track, video_track],
                ice_servers: Vec::new(),
                force_relay: false,
            })
            .unwrap();

        delay_for(200).await;
        match commands_rx.next().await.unwrap() {
            Command::MakeSdpOffer {
                peer_id,
                sdp_offer: _,
                mids,
                transceivers_statuses,
            } => {
                assert_eq!(peer_id, PeerId(1));
                assert_eq!(mids.len(), 2);
                let audio = transceivers_statuses.get(&TrackId(1)).unwrap();
                let video = transceivers_statuses.get(&TrackId(2)).unwrap();

                assert!(audio); // not disabled
                assert!(!video); // disabled
            }
            _ => unreachable!(),
        }

        let peer = room.get_peer_by_id(PeerId(1)).unwrap();
        assert!(!peer.is_send_video_enabled(None));
        assert!(peer.is_send_audio_enabled());
    }
}

/// Tests for `RoomHandle.on_close` JS side callback.
mod on_close_callback {
    use medea_client_api_proto::CloseReason as CloseByServerReason;
    use medea_jason::rpc::{ClientDisconnect, CloseReason};
    use wasm_bindgen::{prelude::*, JsValue};
    use wasm_bindgen_test::*;

    use super::*;

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
        let (room, _) = get_test_room(stream::pending().boxed());
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
        let (room, _) = get_test_room(stream::pending().boxed());
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
        let (room, _) = get_test_room(stream::pending().boxed());
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

        let (_event_tx, event_rx) = mpsc::unbounded();
        rpc.expect_subscribe()
            .return_once(move || Box::pin(event_rx));
        rpc.expect_send_command().return_const(());
        rpc.expect_unsub().return_const(());
        rpc.expect_on_connection_loss()
            .return_once(|| stream::pending().boxed_local());
        rpc.expect_on_reconnected()
            .return_once(|| stream::pending().boxed_local());
        let (test_tx, test_rx) = oneshot::channel();
        rpc.expect_set_close_reason().return_once(move |reason| {
            test_tx.send(reason).unwrap();
        });
        let room = Room::new(Rc::new(rpc), Box::new(MockPeerRepository::new()));
        (room, test_rx)
    }

    /// Tests that [`Room`] sets right [`ClientDisconnect`] close reason on
    /// unexpected drop.
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

    use futures::StreamExt;
    use medea_client_api_proto::{
        AudioSettings, Direction, MediaSourceKind, MediaType, Track, TrackId,
        TrackPatchCommand, VideoSettings,
    };
    use medea_jason::{
        media::RecvConstraints,
        peer::{MediaState, StableMediaExchangeState, StableMuteState},
        JsMediaSourceKind,
    };
    use wasm_bindgen_futures::spawn_local;

    use crate::{is_firefox, timeout};

    use super::*;

    fn audio_and_device_video_tracks_content() -> Vec<(MediaType, Direction)> {
        vec![
            (
                MediaType::Audio(AudioSettings { is_required: false }),
                Direction::Send {
                    receivers: Vec::new(),
                    mid: None,
                },
            ),
            (
                MediaType::Video(VideoSettings {
                    is_required: false,
                    source_kind: MediaSourceKind::Device,
                }),
                Direction::Send {
                    receivers: Vec::new(),
                    mid: None,
                },
            ),
        ]
    }

    /// Returns [`Room`] with mocked [`PeerRepository`] with provided count of
    /// [`PeerConnection`]s and [`mpsc::UnboundedReceiver`] of [`Command`]s
    /// sent from this [`Room`].
    ///
    /// `audio_track_media_state_fn`'s output will be used as `media_state`
    /// value for all audio [`Track`]s.
    async fn get_room_and_commands_receiver(
        peers_count: u32,
        audio_track_media_state_fn: impl Fn(u32) -> MediaState,
        tracks_content: Vec<(MediaType, Direction)>,
    ) -> (Room, mpsc::UnboundedReceiver<Command>) {
        let mut repo = Box::new(MockPeerRepository::new());

        let mut peers = HashMap::new();
        for i in 0..peers_count {
            let (tx, _rx) = mpsc::unbounded();
            let tracks = tracks_content
                .iter()
                .enumerate()
                .map(|(track_i, (media_type, direction))| Track {
                    id: TrackId(track_i as u32),
                    direction: direction.clone(),
                    media_type: media_type.clone(),
                })
                .collect();
            let peer_id = PeerId(i + 1);

            let mut local_stream = MediaStreamSettings::default();
            local_stream.set_track_media_state(
                (audio_track_media_state_fn)(i),
                MediaKind::Audio,
                None,
            );
            let peer = PeerConnection::new(
                peer_id,
                tx,
                Vec::new(),
                Rc::new(MediaManager::default()),
                false,
                local_stream.into(),
                Rc::new(RecvConstraints::default()),
            )
            .unwrap();

            peer.get_offer(tracks).await.unwrap();

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
        rpc.expect_on_connection_loss()
            .return_once(|| stream::pending().boxed_local());
        rpc.expect_on_reconnected()
            .return_once(|| stream::pending().boxed_local());

        (Room::new(Rc::new(rpc), repo), command_rx)
    }

    /// Tests that [`Room`] normally generates [`TrackPatch`]s when have one
    /// [`PeerConnection`] with one enabled video [`Track`] and one enabled
    /// audio [`Track`].
    ///
    /// # Algorithm
    ///
    /// 1. Get mock of [`Room`] and [`Command`]s receiver of this [`Room`] with
    ///    one [`PeerConnection`]s.
    ///
    /// 2. Call [`RoomHandle::disable_audio`].
    ///
    /// 3. Check that [`Room`] tries to send one [`Command::UpdateTracks`] with
    ///    one [`TrackPatch`] for audio [`Track`].
    #[wasm_bindgen_test]
    async fn track_patch_for_all_video() {
        let (room, mut command_rx) = get_room_and_commands_receiver(
            1,
            |_| StableMediaExchangeState::Enabled.into(),
            audio_and_device_video_tracks_content(),
        )
        .await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.disable_audio())
                .await
                .unwrap_err();
        });

        assert_eq!(
            command_rx.next().await.unwrap(),
            Command::UpdateTracks {
                peer_id: PeerId(1),
                tracks_patches: vec![TrackPatchCommand {
                    id: TrackId(0),
                    is_disabled: Some(true),
                    is_muted: None,
                }]
            }
        );
    }

    /// Tests that [`Room`] normally generates [`TrackPatch`]s when have two
    /// [`PeerConnection`] with one enabled video [`Track`] and one enabled
    /// audio [`Track`] in both [`PeerConnection`]s.
    ///
    /// # Algorithm
    ///
    /// 1. Get mock of [`Room`] and [`Command`]s receiver of this [`Room`] with
    ///    two [`PeerConnection`]s.
    ///
    /// 2. Call [`RoomHandle::disable_audio`].
    ///
    /// 3. Check that [`Room`] tries to send two [`Command::UpdateTracks`] for
    ///    enabled [`PeerConnection`]s. [`PeerConnection`]s.
    #[wasm_bindgen_test]
    async fn track_patch_for_many_tracks() {
        let (room, mut command_rx) = get_room_and_commands_receiver(
            2,
            |_| StableMediaExchangeState::Enabled.into(),
            audio_and_device_video_tracks_content(),
        )
        .await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.disable_audio())
                .await
                .unwrap_err();
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
            vec![TrackPatchCommand {
                id: TrackId(0),
                is_disabled: Some(true),
                is_muted: None,
            }]
        );

        assert_eq!(
            commands.remove(&PeerId(2)).unwrap(),
            vec![TrackPatchCommand {
                id: TrackId(0),
                is_disabled: Some(true),
                is_muted: None,
            }]
        );
    }

    /// Tests that [`Room`] wouldn't generate [`TrackPatch`]s for already
    /// enabled [`PeerConnection`]s.
    ///
    /// # Algorithm
    ///
    /// 1. Get mock of [`Room`] and [`Command`]s receiver of this [`Room`] with
    ///    two [`PeerConnection`]s.
    ///
    /// 2. Call [`RoomHandle::enable_audio`].
    ///
    /// 3. Check that [`Room`] doesn't send [`Command::UpdateTracks`] with
    ///    [`RpcClient`].
    #[wasm_bindgen_test]
    async fn try_to_enable_enabled() {
        let (room, mut command_rx) = get_room_and_commands_receiver(
            2,
            |_| StableMediaExchangeState::Enabled.into(),
            audio_and_device_video_tracks_content(),
        )
        .await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.enable_audio()).await.unwrap();
        });

        assert!(timeout(5, command_rx.next()).await.is_err());
    }

    /// Tests that [`Room`] will generate [`Command::UpdateTracks`] only for
    /// enabled [`PeerConnection`].
    ///
    /// # Algorithm
    ///
    /// 1. Get mock of [`Room`] and [`Command`]s receiver of this [`Room`] with
    ///    one enabled [`PeerConnection`]s and one disabled [`PeerConnection`].
    ///
    /// 2. Call [`RoomHandle::disable_audio`].
    ///
    /// 3. Check that [`Room`] tries to send [`Command::UpdateTracks`] only for
    ///    enabled [`PeerConnection`].
    #[wasm_bindgen_test]
    async fn disable_room_with_one_disabled_track() {
        let (room, mut command_rx) = get_room_and_commands_receiver(
            2,
            |i| {
                if i % 2 == 1 {
                    StableMediaExchangeState::Enabled.into()
                } else {
                    StableMediaExchangeState::Disabled.into()
                }
            },
            audio_and_device_video_tracks_content(),
        )
        .await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.disable_audio())
                .await
                .unwrap_err();
        });

        assert_eq!(
            command_rx.next().await.unwrap(),
            Command::UpdateTracks {
                peer_id: PeerId(2),
                tracks_patches: vec![TrackPatchCommand {
                    id: TrackId(0),
                    is_disabled: Some(true),
                    is_muted: None,
                }]
            }
        );
    }

    /// Checks that on device video muting, correct [`Command::UpdateTracks`]
    /// will be sent to the `Media Server`.
    ///
    /// This test will be ignore in Firefox browser.
    #[wasm_bindgen_test]
    async fn disable_device_video() {
        if is_firefox() {
            return;
        }

        let mut tracks = audio_and_device_video_tracks_content();
        tracks.push((
            MediaType::Video(VideoSettings {
                source_kind: MediaSourceKind::Display,
                is_required: false,
            }),
            Direction::Send {
                mid: None,
                receivers: vec![],
            },
        ));
        let (room, command_rx) = get_room_and_commands_receiver(
            2,
            |_| StableMediaExchangeState::Enabled.into(),
            tracks,
        )
        .await;

        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(
                room_handle.disable_video(Some(JsMediaSourceKind::Device)),
            )
            .await
            .unwrap_err();
        });

        let commands: Vec<_> = command_rx.take(2).collect().await;
        for command in commands {
            match command {
                Command::UpdateTracks { tracks_patches, .. } => assert_eq!(
                    tracks_patches,
                    vec![TrackPatchCommand {
                        id: TrackId(1),
                        is_disabled: Some(true),
                        is_muted: None,
                    }]
                ),
                _ => {
                    unreachable!("unexpected command");
                }
            }
        }
    }

    /// Checks that on display video muting, correct [`Command::UpdateTracks`]
    /// will be sent to the `Media Server`.
    ///
    /// This test will be ignore in Firefox browser.
    #[wasm_bindgen_test]
    async fn disable_display_video() {
        if is_firefox() {
            return;
        }

        let mut tracks = audio_and_device_video_tracks_content();
        tracks.push((
            MediaType::Video(VideoSettings {
                source_kind: MediaSourceKind::Display,
                is_required: false,
            }),
            Direction::Send {
                mid: None,
                receivers: vec![],
            },
        ));
        let (room, command_rx) = get_room_and_commands_receiver(
            2,
            |_| StableMediaExchangeState::Enabled.into(),
            tracks,
        )
        .await;

        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(
                room_handle.disable_video(Some(JsMediaSourceKind::Display)),
            )
            .await
            .unwrap_err();
        });

        let commands: Vec<_> = command_rx.take(2).collect().await;
        for command in commands {
            match command {
                Command::UpdateTracks { tracks_patches, .. } => assert_eq!(
                    tracks_patches,
                    vec![TrackPatchCommand {
                        id: TrackId(2),
                        is_disabled: Some(true),
                        is_muted: None,
                    }]
                ),
                _ => {
                    unreachable!("unexpected command");
                }
            }
        }
    }

    /// Checks that correct [`TrackPatchCommand`] generated on muting.
    #[wasm_bindgen_test]
    async fn track_patch_on_muting() {
        let (room, mut command_rx) = get_room_and_commands_receiver(
            1,
            |_| StableMuteState::Unmuted.into(),
            audio_and_device_video_tracks_content(),
        )
        .await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.mute_audio()).await.unwrap_err();
        });

        assert_eq!(
            command_rx.next().await.unwrap(),
            Command::UpdateTracks {
                peer_id: PeerId(1),
                tracks_patches: vec![TrackPatchCommand {
                    id: TrackId(0),
                    is_disabled: None,
                    is_muted: Some(true),
                }]
            }
        );
    }

    /// Checks that correct [`TrackPatchCommand`] generated on unmuting.
    #[wasm_bindgen_test]
    async fn track_patch_on_unmuting() {
        let (room, mut command_rx) = get_room_and_commands_receiver(
            1,
            |_| StableMuteState::Muted.into(),
            audio_and_device_video_tracks_content(),
        )
        .await;
        let room_handle = room.new_handle();

        spawn_local(async move {
            JsFuture::from(room_handle.unmute_audio())
                .await
                .unwrap_err();
        });

        assert_eq!(
            command_rx.next().await.unwrap(),
            Command::UpdateTracks {
                peer_id: PeerId(1),
                tracks_patches: vec![TrackPatchCommand {
                    id: TrackId(0),
                    is_disabled: None,
                    is_muted: Some(false),
                }]
            }
        );
    }
}

/// Checks that muting and unmuting of audio works.
#[wasm_bindgen_test]
async fn mute_unmute_audio() {
    let (audio_track, video_track) = get_test_tracks(false, false);
    let (room, peer) = get_test_room_and_exist_peer(
        vec![audio_track, video_track],
        Some(media_stream_settings(true, true)),
    )
    .await;

    let handle = room.new_handle();
    assert!(JsFuture::from(handle.mute_audio()).await.is_ok());
    assert!(!peer.is_send_audio_unmuted());
    assert!(JsFuture::from(handle.unmute_audio()).await.is_ok());
    assert!(peer.is_send_audio_unmuted());
}

/// Tests that disabling and enabling of remote audio works.
#[wasm_bindgen_test]
async fn remote_disable_enable_audio() {
    let (audio_track, video_track) = get_test_recv_tracks();
    let (room, peer) = get_test_room_and_exist_peer(
        vec![audio_track, video_track],
        Some(media_stream_settings(true, true)),
    )
    .await;

    let handle = room.new_handle();
    assert!(JsFuture::from(handle.disable_remote_audio()).await.is_ok());
    assert!(!peer.is_recv_audio_enabled());
    assert!(JsFuture::from(handle.enable_remote_audio()).await.is_ok());
    assert!(peer.is_recv_audio_enabled());
}

/// Tests that disabling and enabling of remote video works.
#[wasm_bindgen_test]
async fn remote_disable_enable_video() {
    let (audio_track, video_track) = get_test_recv_tracks();
    let (room, peer) = get_test_room_and_exist_peer(
        vec![audio_track, video_track],
        Some(media_stream_settings(true, true)),
    )
    .await;

    let handle = room.new_handle();
    assert!(JsFuture::from(handle.disable_remote_video()).await.is_ok());
    assert!(!peer.is_recv_video_enabled());
    assert!(JsFuture::from(handle.enable_remote_video()).await.is_ok());
    assert!(peer.is_recv_video_enabled());
}

/// Tests that calling [`RoomHandle::set_local_media_settings`] updates needed
/// [`MediaExchangeState`]s of the [`Sender`]s.
#[wasm_bindgen_test]
async fn set_local_media_stream_settings_updates_media_exchange_state() {
    let (event_tx, event_rx) = mpsc::unbounded();
    let (room, mut commands_rx) = get_test_room(Box::pin(event_rx));
    let room_handle = room.new_handle();
    room_handle
        .on_failed_local_media(js_sys::Function::new_no_args(""))
        .unwrap();
    JsFuture::from(
        room_handle
            .set_local_media_settings(&media_stream_settings(true, false)),
    )
    .await
    .unwrap();

    let (audio_track, video_track) = get_test_unrequired_tracks();
    event_tx
        .unbounded_send(Event::PeerCreated {
            peer_id: PeerId(1),
            negotiation_role: NegotiationRole::Offerer,
            tracks: vec![audio_track, video_track],
            ice_servers: Vec::new(),
            force_relay: false,
        })
        .unwrap();
    delay_for(10).await;

    spawn_local(async move {
        JsFuture::from(
            room_handle
                .set_local_media_settings(&media_stream_settings(true, true)),
        )
        .await
        .unwrap_err();
    });
    yield_now().await;

    while let Some(update_tracks_cmd) = commands_rx.next().await {
        if let Command::UpdateTracks {
            peer_id,
            mut tracks_patches,
        } = update_tracks_cmd
        {
            assert_eq!(peer_id, PeerId(1));
            let track_patch = tracks_patches.pop().unwrap();
            assert_eq!(track_patch.is_disabled, Some(false));
            assert!(tracks_patches.is_empty());
            break;
        }
    }
}
