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

//#[wasm_bindgen_test(async)]
// fn mute_audio() -> impl Future<Item = (), Error = JsValue> {
//    let (tx, _rx) = unbounded();
//    let manager = Rc::new(MediaManager::default());
//    let (audio_track, video_track) = get_test_tracks();
//    let peer = Connection::new(1, tx, vec![], manager).unwrap();
//    peer.get_offer(vec![audio_track, video_track])
//        .and_then(|_| peer.mute_audio().map(move |_| peer))
//        .map(|peer| assert!(!peer.enabled_audio().unwrap()))
//        .map_err(Into::into)
//}
//
//#[wasm_bindgen_test(async)]
// fn unmute_audio() -> impl Future<Item = (), Error = JsValue> {
//    let (tx, _rx) = unbounded();
//    let manager = Rc::new(MediaManager::default());
//    let (audio_track, video_track) = get_test_tracks();
//    let peer = Connection::new(1, tx, vec![], manager).unwrap();
//    peer.get_offer(vec![audio_track, video_track])
//        .and_then(|_| {
//            peer.mute_audio()
//                .map(|_| peer.unmute_audio())
//                .map(move |_| peer)
//        })
//        .map(|peer| assert!(peer.enabled_audio().unwrap()))
//        .map_err(Into::into)
//}
//
//#[wasm_bindgen_test(async)]
// fn mute_video() -> impl Future<Item = (), Error = JsValue> {
//    let (tx, _rx) = unbounded();
//    let manager = Rc::new(MediaManager::default());
//    let (audio_track, video_track) = get_test_tracks();
//    let peer = Connection::new(1, tx, vec![], manager).unwrap();
//    peer.get_offer(vec![audio_track, video_track])
//        .and_then(|_| peer.mute_video().map(move |_| peer))
//        .map(|peer| assert!(!peer.enabled_video().unwrap()))
//        .map_err(Into::into)
//}
//
//#[wasm_bindgen_test(async)]
// fn unmute_video() -> impl Future<Item = (), Error = JsValue> {
//    let (tx, _rx) = unbounded();
//    let manager = Rc::new(MediaManager::default());
//    let (audio_track, video_track) = get_test_tracks();
//    let peer = Connection::new(1, tx, vec![], manager).unwrap();
//    peer.get_offer(vec![audio_track, video_track])
//        .and_then(|_| {
//            peer.mute_video()
//                .map(|_| peer.unmute_video())
//                .map(move |_| peer)
//        })
//        .map(|peer| assert!(peer.enabled_video().unwrap()))
//        .map_err(Into::into)
//}
