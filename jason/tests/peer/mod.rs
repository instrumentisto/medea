#![cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

use futures::{
    future::{self, IntoFuture},
    sync::mpsc::{unbounded, UnboundedReceiver},
    Future, Stream,
};

use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use medea_client_api_proto::PeerId;
use medea_jason::{
    media::MediaManager,
    peer::{PeerConnection, PeerEvent},
};

use crate::{get_test_tracks, resolve_after};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test(async)]
fn add_candidates_to_answerer_before_offer(
) -> impl Future<Item = (), Error = JsValue> {
    let (tx1, rx1) = unbounded();
    let (tx2, _) = unbounded();

    let manager = Rc::new(MediaManager::default());
    let pc1 = Rc::new(
        PeerConnection::new(PeerId(1), tx1, vec![], Rc::clone(&manager))
            .unwrap(),
    );
    let pc2 =
        Rc::new(PeerConnection::new(PeerId(2), tx2, vec![], manager).unwrap());
    let (audio_track, video_track) = get_test_tracks();

    let pc2_clone = Rc::clone(&pc2);
    let pc2_clone2 = Rc::clone(&pc2);
    pc1.get_offer(vec![audio_track, video_track])
        .then(move |offer| {
            let offer = offer.unwrap();
            handle_ice_candidates(rx1, pc2, 1)
                .then(move |_| {
                    // assert that pc2 has buffered candidates
                    assert!(pc2_clone.candidates_buffer_len() > 0);
                    // then set its remote description
                    pc2_clone.process_offer(offer, vec![])
                })
                .map(move |_| {
                    // and assert that buffer was flushed
                    assert_eq!(pc2_clone2.candidates_buffer_len(), 0);
                })
        })
        .map(move |_| {
            // move so it wont be dropped
            let _ = pc1;
        })
        .map_err(|err| err.into())
}

#[wasm_bindgen_test(async)]
fn add_candidates_to_offerer_before_answer(
) -> impl Future<Item = (), Error = JsValue> {
    let (tx1, _) = unbounded();
    let (tx2, rx2) = unbounded();

    let manager = Rc::new(MediaManager::default());
    let pc1 = Rc::new(
        PeerConnection::new(PeerId(1), tx1, vec![], Rc::clone(&manager))
            .unwrap(),
    );
    let pc2 =
        Rc::new(PeerConnection::new(PeerId(2), tx2, vec![], manager).unwrap());
    let (audio_track, video_track) = get_test_tracks();

    let pc1_clone = Rc::clone(&pc1);
    let pc2_clone = Rc::clone(&pc2);

    pc1.get_offer(vec![audio_track, video_track])
        .then(move |of| {
            pc2_clone.process_offer(of.unwrap(), vec![]).then(move |r| {
                r.unwrap();
                pc2_clone.create_and_set_answer()
            })
        })
        .then(move |answer| {
            let answer = answer.unwrap();
            handle_ice_candidates(rx2, Rc::clone(&pc1_clone), 1).then(
                move |_| {
                    // assert that pc1 has buffered candidates
                    assert!(pc1_clone.candidates_buffer_len() > 0);
                    pc1_clone.set_remote_answer(answer).then(move |_| {
                        // assert that pc1 has buffered candidates got fulshed
                        assert_eq!(pc1_clone.candidates_buffer_len(), 0);
                        Ok(())
                    })
                },
            )
        })
        .map(move |_| {
            let _ = pc1;
            let _ = pc2;
        })
}

#[wasm_bindgen_test(async)]
fn normal_exchange_of_candidates() -> impl Future<Item = (), Error = JsValue> {
    let (tx1, rx1) = unbounded();
    let (tx2, rx2) = unbounded();

    let manager = Rc::new(MediaManager::default());
    let peer1 = Rc::new(
        PeerConnection::new(PeerId(1), tx1, vec![], Rc::clone(&manager))
            .unwrap(),
    );
    let peer2 =
        Rc::new(PeerConnection::new(PeerId(2), tx2, vec![], manager).unwrap());
    let (audio_track, video_track) = get_test_tracks();

    let peer1_clone = Rc::clone(&peer1);
    let peer2_clone = Rc::clone(&peer2);
    spawn_local({
        let peer1_lock = Rc::clone(&peer1);
        let peer2_lock = Rc::clone(&peer2);
        peer1
            .get_offer(vec![audio_track, video_track])
            .and_then(move |offer| {
                let (audio_track, video_track) = get_test_tracks();
                peer2
                    .process_offer(offer, vec![audio_track, video_track])
                    .and_then(move |_| peer2.create_and_set_answer())
                    .and_then(move |answer| peer1.set_remote_answer(answer))
            })
            .map_err(|_| ())
            .and_then(|_| resolve_after(500).map_err(|_| ()))
            .map(move |_| {
                let _ = peer1_lock;
                let _ = peer2_lock;
            })
    });

    future::join_all(vec![
        handle_ice_candidates(rx1, peer2_clone, 2),
        handle_ice_candidates(rx2, peer1_clone, 2),
    ])
    .map(|_| ())
}

fn handle_ice_candidates(
    candidates_rx: UnboundedReceiver<PeerEvent>,
    peer: Rc<PeerConnection>,
    count: u8,
) -> impl Future<Item = (), Error = JsValue> {
    let added = Rc::new(RefCell::new(0));
    candidates_rx
        .for_each(move |event| match event {
            PeerEvent::IceCandidateDiscovered {
                peer_id: _,
                candidate,
                sdp_m_line_index,
                sdp_mid,
            } => {
                let added = Rc::clone(&added);
                peer.add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
                    .map_err(|_| assert!(false))
                    .then(move |_| {
                        *added.borrow_mut() += 1;
                        if *added.borrow() == count {
                            Err(())
                        } else {
                            Ok(())
                        }
                    })
            }
            _ => unreachable!(),
        })
        .into_future()
        .then(|_| Ok(()))
}
