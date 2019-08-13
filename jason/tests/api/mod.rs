#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use futures::{sync::mpsc::unbounded, Future};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use jason::{
    api::Room,
    media::MediaManager,
    peer::{PeerConnection, PeerRepository},
    rpc::MockRpcClient,
};

use crate::get_test_tracks;

wasm_bindgen_test_configure!(run_in_browser);

fn get_test_room_and_peer() -> (Room, Rc<PeerConnection>) {
    let media_manager = Rc::new(MediaManager::default());
    let mut rpc = MockRpcClient::new();
    let mut repo = PeerRepository::default();
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
    let (room, peer) = get_test_room_and_peer();
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
    let (room, peer) = get_test_room_and_peer();
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
