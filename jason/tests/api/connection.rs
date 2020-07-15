#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;

use futures::{
    channel::{mpsc, oneshot},
    stream::LocalBoxStream,
};
use medea_client_api_proto::{
    Direction, MediaType, PeerId, Track, TrackId, VideoSettings,
};
use medea_jason::{
    api::{ConnectionHandle, Connections},
    media::{MediaManager, MediaStreamTrack, TrackKind},
    peer::{
        MuteStateUpdate, MuteStateUpdatesPublisher, PeerConnection,
        RemoteMediaStream, RtcPeerConnection, StableMuteState,
    },
    AudioTrackConstraints, DeviceVideoTrackConstraints, MediaStreamSettings,
};
use wasm_bindgen::{closure::Closure, JsValue};
use wasm_bindgen_test::*;

use crate::{timeout, wait_and_check_test_result};

wasm_bindgen_test_configure!(run_in_browser);

fn proto_recv_video_track() -> Track {
    Track {
        id: TrackId(1),
        direction: Direction::Recv {
            sender: PeerId(234),
            mid: None,
        },
        media_type: MediaType::Video(VideoSettings { is_required: false }),
    }
}

async fn get_video_track() -> MediaStreamTrack {
    let manager = MediaManager::default();
    let mut settings = MediaStreamSettings::new();
    settings.device_video(DeviceVideoTrackConstraints::new());
    let (stream, _) = manager.get_stream(settings).await.unwrap();
    stream.into_tracks().into_iter().next().unwrap()
}

async fn get_audio_track() -> MediaStreamTrack {
    let manager = MediaManager::default();
    let mut settings = MediaStreamSettings::new();
    settings.audio(AudioTrackConstraints::new());
    let (stream, _) = manager.get_stream(settings).await.unwrap();
    stream.into_tracks().into_iter().next().unwrap()
}

#[wasm_bindgen_test]
async fn on_new_connection_fires() {
    let cons = Connections::default();

    let (cb, test_result) = js_callback!(|handle: ConnectionHandle| {
        cb_assert_eq!(handle.get_remote_id().unwrap(), 234);
    });
    cons.on_new_connection(cb.into());

    cons.create_connections_from_tracks(
        PeerId(1),
        &MuteStateUpdatePublisherMock::new(),
        &[proto_recv_video_track()],
    );

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn on_remote_stream_fires() {
    let cons = Connections::default();

    cons.create_connections_from_tracks(
        PeerId(1),
        &MuteStateUpdatePublisherMock::new(),
        &[proto_recv_video_track()],
    );

    let con = cons.get(PeerId(234)).unwrap();
    let con_handle = con.new_handle();

    let (cb, test_result) = js_callback!(|stream: RemoteMediaStream| {
        cb_assert_eq!(
            stream
                .get_media_stream()
                .unwrap()
                .get_video_tracks()
                .length(),
            1
        );
    });
    con_handle.on_remote_stream(cb.into()).unwrap();

    con.add_remote_track(TrackId(1), get_video_track().await);

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn tracks_are_added_to_remote_stream() {
    let cons = Connections::default();

    cons.create_connections_from_tracks(
        PeerId(1),
        &MuteStateUpdatePublisherMock::new(),
        &[proto_recv_video_track()],
    );

    let con = cons.get(PeerId(234)).unwrap();
    let con_handle = con.new_handle();

    let (tx, rx) = oneshot::channel();
    let closure = Closure::once_into_js(move |stream: RemoteMediaStream| {
        assert!(tx.send(stream).is_ok());
    });
    con_handle.on_remote_stream(closure.into()).unwrap();

    con.add_remote_track(TrackId(1), get_video_track().await);

    let stream = timeout(100, rx).await.unwrap().unwrap();
    let stream = stream.get_media_stream().unwrap();
    assert_eq!(stream.get_tracks().length(), 1);

    con.add_remote_track(TrackId(2), get_audio_track().await);
    assert_eq!(stream.get_tracks().length(), 2);
}

struct MuteStateUpdatePublisherMock {
    tx: RefCell<Vec<mpsc::UnboundedSender<MuteStateUpdate>>>,
}

impl MuteStateUpdatePublisherMock {
    pub fn new() -> Self {
        Self {
            tx: RefCell::default(),
        }
    }

    pub fn send_mute_state(&self, update: MuteStateUpdate) {
        for tx in self.tx.borrow().iter() {
            tx.unbounded_send(update.clone());
        }
    }
}

impl MuteStateUpdatesPublisher for MuteStateUpdatePublisherMock {
    fn on_mute_state_update(&self) -> LocalBoxStream<'static, MuteStateUpdate> {
        let (tx, rx) = mpsc::unbounded();
        self.tx.borrow_mut().push(tx);

        Box::pin(rx)
    }
}

#[wasm_bindgen_test]
async fn on_closed_fires() {
    let cons = Connections::default();
    cons.create_connections_from_tracks(
        PeerId(1),
        &MuteStateUpdatePublisherMock::new(),
        &[proto_recv_video_track()],
    );
    let con = cons.get(PeerId(234)).unwrap();
    let con_handle = con.new_handle();

    let (on_close, test_result) = js_callback!(|nothing: JsValue| {
        cb_assert_eq!(nothing.is_undefined(), true);
    });
    con_handle.on_close(on_close.into()).unwrap();

    cons.close_connection(PeerId(1));

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn on_track_added_works() {
    let cons = Connections::default();

    let mute_state_update_publisher = MuteStateUpdatePublisherMock::new();
    cons.create_connections_from_tracks(
        PeerId(1),
        &mute_state_update_publisher,
        &[proto_recv_video_track()],
    );
    let conn = cons.get(PeerId(234)).unwrap();

    let conn_handle = conn.new_handle();
    let (remote_stream_tx, remote_stream_rx) = oneshot::channel();

    conn_handle
        .on_remote_stream(
            Closure::once_into_js(move |remote_stream: RemoteMediaStream| {
                let _ = remote_stream_tx.send(remote_stream);
            })
            .into(),
        )
        .unwrap();
    conn.add_remote_track(TrackId(1), get_audio_track().await);

    let remote_stream: RemoteMediaStream = remote_stream_rx.await.unwrap();
    let (on_track_started, test_result) = js_callback!(|kind: String| {
        cb_assert_eq!(kind, "audio".to_string());
    });

    remote_stream
        .on_track_added(on_track_started.into())
        .unwrap();

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn on_track_stopped_works() {
    let cons = Connections::default();

    let mute_state_update_publisher = MuteStateUpdatePublisherMock::new();
    cons.create_connections_from_tracks(
        PeerId(1),
        &mute_state_update_publisher,
        &[proto_recv_video_track()],
    );
    let conn = cons.get(PeerId(234)).unwrap();

    let conn_handle = conn.new_handle();
    let (remote_stream_tx, remote_stream_rx) = oneshot::channel();

    conn_handle
        .on_remote_stream(
            Closure::once_into_js(move |remote_stream: RemoteMediaStream| {
                let _ = remote_stream_tx.send(remote_stream);
            })
            .into(),
        )
        .unwrap();
    conn.add_remote_track(TrackId(1), get_audio_track().await);

    let remote_stream: RemoteMediaStream = remote_stream_rx.await.unwrap();
    let (on_track_started, test_result) = js_callback!(|kind: String| {
        cb_assert_eq!(kind, "audio".to_string());
    });

    remote_stream
        .on_track_stopped(on_track_started.into())
        .unwrap();

    mute_state_update_publisher.send_mute_state(MuteStateUpdate {
        kind: TrackKind::Audio,
        new_mute_state: StableMuteState::Muted,
    });

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn on_track_started_works() {
    let cons = Connections::default();

    let mute_state_update_publisher = MuteStateUpdatePublisherMock::new();
    cons.create_connections_from_tracks(
        PeerId(1),
        &mute_state_update_publisher,
        &[proto_recv_video_track()],
    );
    let conn = cons.get(PeerId(234)).unwrap();

    let conn_handle = conn.new_handle();
    let (remote_stream_tx, remote_stream_rx) = oneshot::channel();

    conn_handle
        .on_remote_stream(
            Closure::once_into_js(move |remote_stream: RemoteMediaStream| {
                let _ = remote_stream_tx.send(remote_stream);
            })
            .into(),
        )
        .unwrap();
    conn.add_remote_track(TrackId(1), get_audio_track().await);

    let remote_stream: RemoteMediaStream = remote_stream_rx.await.unwrap();
    let (on_track_started, test_result_on_stop) =
        js_callback!(|kind: String| {
            cb_assert_eq!(kind, "audio".to_string());
        });

    mute_state_update_publisher.send_mute_state(MuteStateUpdate {
        kind: TrackKind::Audio,
        new_mute_state: StableMuteState::Muted,
    });

    remote_stream
        .on_track_stopped(on_track_started.into())
        .unwrap();

    wait_and_check_test_result(test_result_on_stop, || {}).await;

    let (on_track_started, test_result_on_start) =
        js_callback!(|kind: String| {
            cb_assert_eq!(kind, "audio".to_string());
        });
    remote_stream
        .on_track_started(on_track_started.into())
        .unwrap();

    mute_state_update_publisher.send_mute_state(MuteStateUpdate {
        kind: TrackKind::Audio,
        new_mute_state: StableMuteState::NotMuted,
    });

    wait_and_check_test_result(test_result_on_start, || {}).await;
}
