#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};

use futures::channel::mpsc;
use medea_client_api_proto::{CloseReason, Event, IceServer, PeerId};
use medea_jason::{
    api::Room,
    media::MediaManager,
    peer::{MockPeerRepository, PeerConnection, PeerEvent},
    rpc::MockRpcClient,
};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use crate::{get_test_tracks, resolve_after};
use wasm_bindgen::JsValue;

wasm_bindgen_test_configure!(run_in_browser);

fn get_test_room_and_exist_peer() -> (Room, Rc<PeerConnection>) {
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
        .times(2)
        .returning_st(move || vec![Rc::clone(&peer_clone)]);
    rpc.expect_unsub().return_const(());
    rpc.expect_on_close_by_server().returning(move || {
        let (tx, rx) = futures::channel::oneshot::channel();
        Box::pin(rx)
    });

    let room = Room::new(Rc::new(rpc), repo);
    (room, peer)
}

#[wasm_bindgen_test]
async fn mute_unmute_audio() {
    let (room, peer) = get_test_room_and_exist_peer();
    let (audio_track, video_track) = get_test_tracks();

    peer.get_offer(vec![audio_track, video_track])
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
    let (room, peer) = get_test_room_and_exist_peer();
    let (audio_track, video_track) = get_test_tracks();

    peer.get_offer(vec![audio_track, video_track])
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
    rpc.expect_on_close_by_server().returning(move || {
        let (tx, rx) = futures::channel::oneshot::channel();
        Box::pin(rx)
    });

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

#[wasm_bindgen(
inline_js = "export function get_reason(closed) { return closed.reason; }"
)]

extern "C" {
    fn get_reason(&JsValue) -> JsValue;
}

#[wasm_bindgen_test]
async fn close_room() {
    let mut rpc = MockRpcClient::new();
    let mut repo = Box::new(MockPeerRepository::new());

    use futures::channel::oneshot;

    use std::sync::{Arc, Mutex};

    let senders = Arc::new(Mutex::new(Vec::new()));
    let senders_clone = Arc::clone(&senders);
    let (event_tx, event_rx) = mpsc::unbounded();
    rpc.expect_subscribe()
        .return_once(move || Box::pin(event_rx));
    rpc.expect_on_close_by_server().returning(move || {
        let (tx, rx) = oneshot::channel();
        senders_clone.lock().unwrap().push(tx);
        Box::pin(rx)
    });
    rpc.expect_send_command().return_const(());
    rpc.expect_unsub().return_const(());

    use futures::FutureExt;
    use medea_jason::rpc::RpcClient;

    let room = Room::new(Rc::new(rpc), repo);
    let mut room_handle = room.new_handle();
    use wasm_bindgen::{closure::Closure, JsCast};
    console_error_panic_hook::set_once();

    let (mut tx, rx) = futures::channel::oneshot::channel();
    room_handle
        .on_close_by_server(
            Closure::once_into_js(move |close:wasm_bindgen::JsValue| {
                tx.send(());
            }).into(),
        )
        .unwrap();

    let mut sqwe = Vec::new();
    std::mem::swap(&mut sqwe, &mut senders.lock().unwrap());

    for sub in sqwe {
        sub.send(CloseReason::Finished);
    }

    resolve_after(500).await.unwrap();
    let result = futures::future::select(Box::pin(rx), Box::pin(resolve_after(500))).await;

    match result {
        futures::future::Either::Left((assert_result, _)) => {
            if let Err(_) = assert_result {
                panic!("cancelled");
            }
        },
        futures::future::Either::Right(_) => {
            panic!("on_close_by_server did not fired");
        }
    };
}
