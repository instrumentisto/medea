#![cfg(target_arch = "wasm32")]
use std::rc::Rc;

use futures::{
    future::{self, Future as _, IntoFuture},
    sync::mpsc::unbounded,
    Future, Stream,
};

use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use medea_jason::{media::MediaManager, peer::PeerConnection, utils::WasmErr};

use crate::{get_test_tracks, resolve_after};
use futures::future::Either;
use medea_jason::peer::PeerEvent;
use wasm_bindgen_futures::spawn_local;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test(async)]
fn add_ice_candidate_before_precess_offer(
) -> impl Future<Item = (), Error = JsValue> {
    let (tx1, rx1) = unbounded();
    let (tx2, rx2) = unbounded();

    let manager = Rc::new(MediaManager::default());
    let peer1 = Rc::new(
        PeerConnection::new(1, tx1, vec![], Rc::clone(&manager)).unwrap(),
    );
    let peer2 = Rc::new(PeerConnection::new(2, tx2, vec![], manager).unwrap());
    let (audio_track, video_track) = get_test_tracks();

    spawn_local(
        peer1
            .get_offer(vec![audio_track, video_track])
            .map_err(|_| ())
            .and_then(|_| resolve_after(500).map_err(|_| ()))
            .map(move |_| {
                let _ = peer1;
            }),
    );

    rx1.for_each(move |event| match event {
        PeerEvent::IceCandidateDiscovered {
            peer_id: _,
            candidate,
            sdp_m_line_index,
            sdp_mid,
        } => {
            spawn_local(
                peer2
                    .add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                    .map(|_| assert!(false))
                    .map_err(|err| err.log_err()),
            );
            Ok(())
        }
        _ => Ok(()),
    })
    .into_future()
    .then(|_| Ok(()))
}

#[wasm_bindgen_test(async)]
fn add_ice_candidate_before_precess_answer(
) -> impl Future<Item = (), Error = JsValue> {
    let (tx1, rx1) = unbounded();
    let (tx2, rx2) = unbounded();

    let manager = Rc::new(MediaManager::default());
    let peer1 = Rc::new(
        PeerConnection::new(1, tx1, vec![], Rc::clone(&manager)).unwrap(),
    );
    let peer2 = Rc::new(PeerConnection::new(2, tx2, vec![], manager).unwrap());
    let (audio_track, video_track) = get_test_tracks();

    let peer1_clone = Rc::clone(&peer1);
    spawn_local({
        let peer2_clone = Rc::clone(&peer2);
        peer1
            .get_offer(vec![audio_track, video_track])
            .and_then(move |offer| {
                let (audio_track, video_track) = get_test_tracks();
                peer2
                    .process_offer(offer, vec![audio_track, video_track])
                    .and_then(move |_| peer2.create_and_set_answer())
            })
            .map_err(|_| ())
            .and_then(|_| resolve_after(500).map_err(|_| ()))
            .map(move |_| {
                let _ = peer2_clone;
            })
    });

    rx2.for_each(move |event| match event {
        PeerEvent::IceCandidateDiscovered {
            peer_id: _,
            candidate,
            sdp_m_line_index,
            sdp_mid,
        } => {
            spawn_local(
                peer1_clone
                    .add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                    .map(|_| assert!(false))
                    .map_err(|err| err.log_err()),
            );
            Ok(())
        }
        _ => Ok(()),
    })
    .into_future()
    .then(|_| Ok(()))
}
