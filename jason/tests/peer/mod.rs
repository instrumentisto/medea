#![cfg(target_arch = "wasm32")]

use futures::{sync::mpsc::unbounded, Future};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, Track, VideoSettings,
};

use jason::{media::MediaManager, peer::PeerConnection};
use std::rc::Rc;

wasm_bindgen_test_configure!(run_in_browser);

fn get_test_tracks() -> (Track, Track) {
    (
        Track {
            id: 1,
            direction: Direction::Send {
                receivers: vec![2],
                mid: None,
            },
            media_type: MediaType::Audio(AudioSettings {}),
        },
        Track {
            id: 2,
            direction: Direction::Send {
                receivers: vec![2],
                mid: None,
            },
            media_type: MediaType::Video(VideoSettings {}),
        },
    )
}

#[wasm_bindgen_test(async)]
fn mute_audio() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(1, tx, vec![], manager).unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .and_then(|_| peer.mute_audio().map(move |_| peer))
        .map(|peer| assert!(!peer.enabled_audio().unwrap()))
        .map_err(Into::into)
}

#[wasm_bindgen_test(async)]
fn unmute_audio() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(1, tx, vec![], manager).unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .and_then(|_| {
            peer.mute_audio()
                .map(|_| peer.unmute_audio())
                .map(move |_| peer)
        })
        .map(|peer| assert!(peer.enabled_audio().unwrap()))
        .map_err(Into::into)
}

#[wasm_bindgen_test(async)]
fn mute_video() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(1, tx, vec![], manager).unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .and_then(|_| peer.mute_video().map(move |_| peer))
        .map(|peer| assert!(!peer.enabled_video().unwrap()))
        .map_err(Into::into)
}

#[wasm_bindgen_test(async)]
fn unmute_video() -> impl Future<Item = (), Error = JsValue> {
    let (tx, _rx) = unbounded();
    let manager = Rc::new(MediaManager::default());
    let (audio_track, video_track) = get_test_tracks();
    let peer = PeerConnection::new(1, tx, vec![], manager).unwrap();
    peer.get_offer(vec![audio_track, video_track])
        .and_then(|_| {
            peer.mute_video()
                .map(|_| peer.unmute_video())
                .map(move |_| peer)
        })
        .map(|peer| assert!(peer.enabled_video().unwrap()))
        .map_err(Into::into)
}
