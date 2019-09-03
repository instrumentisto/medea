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

use crate::get_test_tracks;
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
    let peer1 =
        PeerConnection::new(1, tx1, vec![], Rc::clone(&manager)).unwrap();
    let peer2 = PeerConnection::new(2, tx2, vec![], manager).unwrap();
    let (audio_track, video_track) = get_test_tracks();
    peer1
        .get_offer(vec![audio_track, video_track])
        .and_then(move |offer| {
            let (audio_track, video_track) = get_test_tracks();
            peer2
                .process_offer(offer, vec![audio_track, video_track])
                .and_then(move |_| {
                    peer2.create_and_set_answer().and_then(move |answer| {
                        peer1.set_remote_answer(answer).and_then(move |_| {
                            rx2.into_future()
                                .map_err(|_| WasmErr::from("Nothing receive"))
                                .and_then(move |(event, _)| match event {
                                    None => {
                                        Either::A(future::ok::<_, WasmErr>(
                                            assert!(false),
                                        ))
                                    }
                                    Some(e) => Either::B(match e {
                                        PeerEvent::IceCandidateDiscovered {
                                            peer_id: _,
                                            candidate,
                                            sdp_m_line_index,
                                            sdp_mid,
                                        } => Either::A(
                                            peer1
                                                .add_ice_candidate(
                                                    candidate,
                                                    sdp_m_line_index,
                                                    sdp_mid,
                                                )
                                                .then(move |res| {
                                                    let _ = peer2;
                                                    match res {
                                                        Ok(()) => future::ok::<
                                                            _,
                                                            WasmErr,
                                                        >(
                                                            assert!(
                                                            true
                                                        )
                                                        ),
                                                        Err(e) => {
                                                            e.log_err();
                                                            future::ok::<
                                                                _,
                                                                WasmErr,
                                                            >(
                                                                assert!(
                                                                false
                                                            )
                                                            )
                                                        }
                                                    }
                                                }),
                                        ),
                                        _ => {
                                            Either::B(future::ok::<_, WasmErr>(
                                                assert!(false),
                                            ))
                                        }
                                    }),
                                })
                        })
                    })
                })
        })
        .map_err(Into::into)
}
