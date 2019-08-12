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

// Wait for https://github.com/rustwasm/wasm-pack/issues/698
// Wait for chrome_args

/// TODO: pass firefoxOptions.prefs to wasp-pack test:
//  let request = json!({
//                    "capabilities": {
//                        "alwaysMatch": {
//                            "moz:firefoxOptions": {
//                                "prefs": {
//                                    "media.navigator.streams.fake": true,
//                                    "media.navigator.permission.disabled": true,
//                                    "media.autoplay.enabled": true,
//                                    "media.autoplay.enabled.user-gestures-needed ": false,
//                                    "media.autoplay.ask-permission": false,
//                                    "media.autoplay.default": 0,
//                                },
//                                "args": args,
//                            }
//                        }
//                    }
//                });

#[wasm_bindgen_test(async)]
 fn mute_unmute_audio() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = Connection::new(1, tx, vec![], manager).unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .map(move |_| {
            assert!(peer.is_send_audio_enabled().unwrap());
            assert!(peer.is_send_video_enabled().unwrap());

            peer.toggle_send_audio(false);
            assert!(!peer.is_send_audio_enabled().unwrap());
            assert!(peer.is_send_video_enabled().unwrap());

            peer.toggle_send_audio(true);
            assert!(peer.is_send_audio_enabled().unwrap());
            assert!(peer.is_send_video_enabled().unwrap());
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
            assert!(peer.is_send_audio_enabled().unwrap());
            assert!(peer.is_send_video_enabled().unwrap());

            peer.toggle_send_video(false);
            assert!(peer.is_send_audio_enabled().unwrap());
            assert!(!peer.is_send_video_enabled().unwrap());

            peer.toggle_send_video(true);
            assert!(peer.is_send_audio_enabled().unwrap());
            assert!(peer.is_send_video_enabled().unwrap());
        })
        .map_err(Into::into)
}