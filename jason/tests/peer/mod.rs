#![cfg(target_arch = "wasm32")]
use std::rc::Rc;

use futures::{sync::mpsc::unbounded, Future};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use medea_client_api_proto::PeerId;
use medea_jason::{media::MediaManager, peer::PeerConnection};

use crate::get_test_tracks;

wasm_bindgen_test_configure!(run_in_browser);

mod media;

/// TODO: enable tests in firefox, PR: rustwasm/wasm-bindgen#1744
// firefoxOptions.prefs:
//    let request = json!({
//        "capabilities": {
//            "alwaysMatch": {
//                "moz:firefoxOptions": {
//                    "prefs": {
//                        "media.navigator.streams.fake": true,
//                        "media.navigator.permission.disabled": true,
//                        "media.autoplay.enabled": true,
//                        "media.autoplay.enabled.user-gestures-needed ": false,
//                        "media.autoplay.ask-permission": false,
//                        "media.autoplay.default": 0,
//                    },
//                    "args": args,
//                }
//            }
//        }
//    });

#[wasm_bindgen_test(async)]
fn mute_unmute_audio() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(PeerId(1), tx, vec![], manager, true, true)
        .unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            assert!(peer.is_send_audio_enabled());
            assert!(peer.is_send_video_enabled());

            peer.toggle_send_audio(false);
            assert!(!peer.is_send_audio_enabled());
            assert!(peer.is_send_video_enabled());

            peer.toggle_send_audio(true);
            assert!(peer.is_send_audio_enabled());
            assert!(peer.is_send_video_enabled());
        })
        .map_err(Into::into)
}

#[wasm_bindgen_test(async)]
fn mute_unmute_video() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(PeerId(1), tx, vec![], manager, true, true)
        .unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            assert!(peer.is_send_audio_enabled());
            assert!(peer.is_send_video_enabled());

            peer.toggle_send_video(false);
            assert!(peer.is_send_audio_enabled());
            assert!(!peer.is_send_video_enabled());

            peer.toggle_send_video(true);
            assert!(peer.is_send_audio_enabled());
            assert!(peer.is_send_video_enabled());
        })
        .map_err(Into::into)
}

#[wasm_bindgen_test(async)]
fn new_with_mute_audio() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(PeerId(1), tx, vec![], manager, false, true)
        .unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            assert!(!peer.is_send_audio_enabled());
            assert!(peer.is_send_video_enabled());
        })
        .map_err(Into::into)
}

#[wasm_bindgen_test(async)]
fn new_with_mute_video() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(PeerId(1), tx, vec![], manager, true, false)
        .unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            assert!(peer.is_send_audio_enabled());
            assert!(!peer.is_send_video_enabled());
        })
        .map_err(Into::into)
}
