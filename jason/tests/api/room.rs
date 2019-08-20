#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use futures::{
    future::IntoFuture,
    sync::{
        mpsc::{unbounded, UnboundedReceiver},
        oneshot::channel,
    },
    Future,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

use jason::{
    api::Room,
    media::MediaManager,
    peer::{self, MockPeerRepository, PeerConnection, PeerRepository},
    rpc::MockRpcClient,
    utils::window,
};
use medea_client_api_proto::Event;

use crate::get_test_tracks;
use futures::sync::oneshot::Canceled;
use jason::utils::WasmErr;

wasm_bindgen_test_configure!(run_in_browser);

fn get_test_room_and_exist_peer() -> (Room, Rc<PeerConnection>) {
    let media_manager = Rc::new(MediaManager::default());
    let mut rpc = MockRpcClient::new();
    let mut repo = Box::new(peer::Repository::default());
    let (tx, _rx) = unbounded();
    let peer = Rc::new(
        PeerConnection::new(1, tx, vec![], Rc::clone(&media_manager)).unwrap(),
    );
    repo.insert(1, Rc::clone(&peer));

    let (_event_sender, event_receiver) = unbounded();
    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), repo, media_manager);
    (room, peer)
}

#[wasm_bindgen_test(async)]
fn mute_unmute_audio() -> impl Future<Item = (), Error = JsValue> {
    let (room, peer) = get_test_room_and_exist_peer();
    let (audio_track, video_track) = get_test_tracks();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            let handle = room.new_handle();
            assert!(handle.mute_audio().is_ok());
            assert!(!peer.is_send_audio_enabled());
            assert!(handle.unmute_audio().is_ok());
            assert!(peer.is_send_audio_enabled());
        })
        .map_err(Into::into)
}

#[wasm_bindgen_test(async)]
fn mute_unmute_video() -> impl Future<Item = (), Error = JsValue> {
    let (room, peer) = get_test_room_and_exist_peer();
    let (audio_track, video_track) = get_test_tracks();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            let handle = room.new_handle();
            assert!(handle.mute_video().is_ok());
            assert!(!peer.is_send_video_enabled());
            assert!(handle.unmute_video().is_ok());
            assert!(peer.is_send_video_enabled());
        })
        .map_err(Into::into)
}

fn get_room_and_new_peer(
    event_receiver: UnboundedReceiver<Event>,
) -> (Room, Rc<PeerConnection>) {
    let media_manager = Rc::new(MediaManager::default());
    let mut rpc = MockRpcClient::new();
    let mut repo = Box::new(MockPeerRepository::new());
    let (tx, _rx) = unbounded();
    let peer = Rc::new(
        PeerConnection::new(1, tx, vec![], Rc::clone(&media_manager)).unwrap(),
    );

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(|| vec![]);
    repo.expect_insert().returning(|_, _| None);
    let peer_clone = Rc::clone(&peer);
    repo.expect_get().return_once_st(move |_| Some(peer_clone));
    rpc.expect_send_command().return_const(());
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), repo, media_manager);
    (room, peer)
}

#[wasm_bindgen_test(async)]
fn mute_audio_room_before_init_peer() -> impl Future<Item = (), Error = JsValue>
{
    let (event_sender, event_receiver) = unbounded();
    let (room, peer) = get_room_and_new_peer(event_receiver);
    let (audio_track, video_track) = get_test_tracks();

    room.new_handle().mute_audio().unwrap();
    event_sender
        .unbounded_send(Event::PeerCreated {
            peer_id: 1,
            sdp_offer: None,
            tracks: vec![audio_track, video_track],
            ice_servers: vec![],
        })
        .unwrap();
    let (done, wait) = channel();
    let cb = Closure::once_into_js(move || {
        done.send("str").unwrap();
    });
    window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            &cb.into(),
            10_000,
        )
        .unwrap();
    wait.into_future()
        .then(move |result: Result<&str, Canceled>| {
            result
                .and_then(move |msg| {
                    WasmErr::from(msg).log_err();
                    assert!(peer.is_send_video_enabled());
                    assert!(!peer.is_send_audio_enabled());
                    Ok(())
                })
                .map_err(|_| WasmErr::from("canceled").into())
        })
}
