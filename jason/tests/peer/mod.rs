#![cfg(target_arch = "wasm32")]
use std::rc::Rc;

use futures::{
    future::{self, IntoFuture},
    sync::mpsc::unbounded,
    Future, Stream,
};

use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use medea_jason::{
    media::MediaManager,
    peer::{PeerConnection, PeerEvent},
};

use crate::{get_test_tracks, resolve_after};
use futures::sync::mpsc::UnboundedReceiver;
use medea_jason::utils::WasmErr;
use std::cell::RefCell;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test(async)]
fn add_ice_candidate_before_process_offer(
) -> impl Future<Item = (), Error = JsValue> {
    let (tx1, rx1) = unbounded();
    let (tx2, _) = unbounded();

    let manager = Rc::new(MediaManager::default());
    let peer1 = Rc::new(
        PeerConnection::new(1, tx1, vec![], Rc::clone(&manager)).unwrap(),
    );
    let peer2 = Rc::new(PeerConnection::new(2, tx2, vec![], manager).unwrap());
    let (audio_track, video_track) = get_test_tracks();

    WasmErr::from("zzzzzzzzzzzzzzzzzzzzzzzzzzzzz").log_err();

//    let peer2_clone = Rc::clone(&peer2);
//    peer1
//        .get_offer(vec![audio_track, video_track])
//        .then(|_| {
//            WasmErr::from("1111111111111111111").log_err();
//            resolve_after(500).map_err(|_| ())
//        })
//        .map(move |_| {
//            WasmErr::from("asdasdasddasdasdasadsa").log_err();
//            assert!(peer2_clone.candidates_buffer_len() == 0)
//        })
//        .then(move |_| handle_ice_candidates(rx1, peer2, 1))

    peer1
        .get_offer(vec![audio_track, video_track])
        .map(|_|())
        .map_err(|err|err.into())
}

//#[wasm_bindgen_test(async)]
// fn add_ice_candidate_before_precess_answer(
//) -> impl Future<Item = (), Error = JsValue> {
//    let (tx1, _) = unbounded();
//    let (tx2, rx2) = unbounded();
//
//    let manager = Rc::new(MediaManager::default());
//    let peer1 = Rc::new(
//        PeerConnection::new(1, tx1, vec![], Rc::clone(&manager)).unwrap(),
//    );
//    let peer2 = Rc::new(PeerConnection::new(2, tx2, vec![],
// manager).unwrap());    let (audio_track, video_track) = get_test_tracks();
//
//    let peer1_clone = Rc::clone(&peer1);
//    let peer2_clone = Rc::clone(&peer2);
//    spawn_local({
//        peer1
//            .get_offer(vec![audio_track, video_track])
//            .and_then(move |offer| {
//                let (audio_track, video_track) = get_test_tracks();
//                peer2
//                    .process_offer(offer, vec![audio_track, video_track])
//                    .and_then(move |_| peer2.create_and_set_answer())
//            })
//            .map_err(|_| ())
//            .and_then(|_| resolve_after(500).map_err(|_| ()))
//            .map(|_| ())
//    });
//
//    handle_ice_candidates(rx2, peer1_clone, 1).map(move |_| {
//        let _ = peer2_clone;
//    })
//}

//#[wasm_bindgen_test(async)]
// fn normal_exchange_of_candidates() -> impl Future<Item = (), Error = JsValue>
// {    let (tx1, rx1) = unbounded();
//    let (tx2, rx2) = unbounded();
//
//    let manager = Rc::new(MediaManager::default());
//    let peer1 = Rc::new(
//        PeerConnection::new(1, tx1, vec![], Rc::clone(&manager)).unwrap(),
//    );
//    let peer2 = Rc::new(PeerConnection::new(2, tx2, vec![],
// manager).unwrap());    let (audio_track, video_track) = get_test_tracks();
//
//    let peer1_clone = Rc::clone(&peer1);
//    let peer2_clone = Rc::clone(&peer2);
//    spawn_local({
//        let peer1_lock = Rc::clone(&peer1);
//        let peer2_lock = Rc::clone(&peer2);
//        peer1
//            .get_offer(vec![audio_track, video_track])
//            .and_then(move |offer| {
//                let (audio_track, video_track) = get_test_tracks();
//                peer2
//                    .process_offer(offer, vec![audio_track, video_track])
//                    .and_then(move |_| peer2.create_and_set_answer())
//                    .and_then(move |answer| peer1.set_remote_answer(answer))
//            })
//            .map_err(|_| ())
//            .and_then(|_| resolve_after(500).map_err(|_| ()))
//            .map(move |_| {
//                let _ = peer1_lock;
//                let _ = peer2_lock;
//            })
//    });
//
//    future::join_all(vec![
//        handle_ice_candidates(rx1, peer2_clone, 2),
//        handle_ice_candidates(rx2, peer1_clone, 2),
//    ])
//    .map(|_| ())
//}

fn handle_ice_candidates(
    rx: UnboundedReceiver<PeerEvent>,
    peer: Rc<PeerConnection>,
    count: u8,
) -> impl Future<Item = (), Error = JsValue> {
    let added = Rc::new(RefCell::new(0));
    rx.for_each(move |event| match event {
        PeerEvent::IceCandidateDiscovered {
            peer_id: _,
            candidate,
            sdp_m_line_index,
            sdp_mid,
        } => {
            if *added.borrow() == count {
                Err(())
            } else {
                let added = Rc::clone(&added);
                spawn_local(
                    peer.add_ice_candidate(
                        candidate,
                        sdp_m_line_index,
                        sdp_mid,
                    )
                    .map_err(|_| assert!(false))
                    .map(move |_| {
                        *added.borrow_mut() += 1;
                    }),
                );
                Ok(())
            }
        }
        _ => Ok(()),
    })
    .into_future()
    .then(|_| Ok(()))
}
