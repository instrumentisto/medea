#![cfg(target_arch = "wasm32")]
use std::rc::Rc;

use futures::Future;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use jason::{
    media::MediaManager,
    peer::{MediaConnections, RtcPeerConnection},
};

use crate::get_test_tracks;
use jason::peer::TransceiverKind;

wasm_bindgen_test_configure!(run_in_browser);

//#[wasm_bindgen_test(async)]
//fn enable_sender_success() -> impl Future<Item = (), Error = JsValue> {
//    let peer = Rc::new(RtcPeerConnection::new(vec![]).unwrap());
//    let media_connections = MediaConnections::new(Rc::clone(&peer));
//    let (audio_track, video_track) = get_test_tracks();
//    let sender = audio_track.id;
//    let request = media_connections
//        .update_tracks(vec![audio_track, video_track])
//        .unwrap()
//        .unwrap();
//    let manager = Rc::new(MediaManager::default());
//    manager
//        .get_stream(request)
//        .and_then(move |stream| {
//            media_connections
//                .insert_local_stream(&stream)
//                .and_then(move |_| {
//                    match media_connections
//                        .enable_sender(TransceiverKind::Audio, false)
//                    {
//                        Ok(()) => {
//                            let tracks = media_connections
//                                .get_tracks_by_sender(sender)
//                                .unwrap();
//                            for track in tracks {
//                                assert!(!track.track().enabled())
//                            }
//                            Ok(())
//                        }
//                        Err(e) => Err(e),
//                    }
//                })
//        })
//        .map_err(Into::into)
//}
//
//#[wasm_bindgen_test]
//fn enable_sender_error() {
//    let peer = Rc::new(RtcPeerConnection::new(vec![]).unwrap());
//    let media_connections = MediaConnections::new(Rc::clone(&peer));
//    let (audio_track, video_track) = get_test_tracks();
//    let _ = media_connections
//        .update_tracks(vec![audio_track, video_track])
//        .unwrap()
//        .unwrap();
//    match media_connections.enable_sender(TransceiverKind::Audio, true) {
//        Ok(()) => assert!(false),
//        Err(e) => assert_eq!("Peer has senders without track", e.to_string()),
//    }
//}
