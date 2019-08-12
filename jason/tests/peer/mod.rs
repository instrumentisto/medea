#![cfg(target_arch = "wasm32")]
use std::rc::Rc;

use futures::{sync::mpsc::unbounded, Future};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use jason::{
    media::MediaManager,
    peer::{Connection, PeerConnection},
};

use crate::get_test_tracks;

wasm_bindgen_test_configure!(run_in_browser);

mod media;

#[wasm_bindgen_test(async)]
 fn mute_unmute_audio() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = Connection::new(1, tx, vec![], manager).unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            assert!(peer.enabled_audio().unwrap());
            assert!(peer.enabled_video().unwrap());

            peer.toggle_send_audio(false);
            assert!(!peer.enabled_audio().unwrap());
            assert!(peer.enabled_video().unwrap());

            peer.toggle_send_audio(true);
            assert!(peer.enabled_audio().unwrap());
            assert!(peer.enabled_video().unwrap());
        })
        .map_err(Into::into)
}

#[wasm_bindgen_test(async)]
fn mute_unmute_video() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = Connection::new(1, tx, vec![], manager).unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            assert!(peer.enabled_audio().unwrap());
            assert!(peer.enabled_video().unwrap());

            peer.toggle_send_video(false);
            assert!(peer.enabled_audio().unwrap());
            assert!(!peer.enabled_video().unwrap());

            peer.toggle_send_video(true);
            assert!(peer.enabled_audio().unwrap());
            assert!(peer.enabled_video().unwrap());
        })
        .map_err(Into::into)
}