#![cfg(target_arch = "wasm32")]

use std::{collections::HashMap, rc::Rc};

use futures::{sync::mpsc::unbounded, Future, Stream};
use mockall::{predicate::*, *};
use wasm_bindgen_test::*;

use medea_client_api_proto::{Command, Event, Track};

use jason::{
    api::Room,
    media::MediaManager,
    peer::{PeerConnection, PeerId, PeerRepository, MockPeerRepository},
    rpc::{RpcClient, MockRpcClient},
    utils::WasmErr,
};

wasm_bindgen_test_configure!(run_in_browser);

mock! {
    PeerConnection {}
    pub trait PeerConnection {
        fn toggle_send_video(&self, enabled: bool);
        fn toggle_send_audio(&self, enabled: bool);
        fn enabled_audio(&self) -> Result<bool, WasmErr>;
        fn enabled_video(&self) -> Result<bool, WasmErr>;
        fn get_mids(&self) -> Result<HashMap<u64, String>, WasmErr>;
        fn get_offer(
            &self,
            tracks: Vec<Track>,
        ) -> Box<dyn Future<Item = String, Error = WasmErr>>;
        fn create_and_set_answer(
            &self,
        ) -> Box<dyn Future<Item = String, Error = WasmErr>>;
        fn set_remote_answer(
            &self,
            answer: String,
        ) -> Box<dyn Future<Item = (), Error = WasmErr>>;
        fn process_offer(
            &self,
            offer: String,
            tracks: Vec<Track>,
        ) -> Box<dyn Future<Item = (), Error = WasmErr>>;
        fn add_ice_candidate(
            &self,
            candidate: &str,
            sdp_m_line_index: Option<u16>,
            sdp_mid: &Option<String>,
        ) -> Box<dyn Future<Item = (), Error = WasmErr>>;
    }
}

#[wasm_bindgen_test]
fn mute_audio_success() {
//    let (_event_sender, event_receiver) = unbounded();
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();
//
//    rpc.expect_subscribe()
//        .return_once(move || Box::new(event_receiver));
//    repo.expect_get_all().returning(move || {
//        let mut peer = MockPeerConnection::new();
//        peer.expect_mute_audio().returning(|| Ok(()));
//        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
//    });
//    rpc.expect_unsub().return_const(());
//
//    let room = Room::new(Rc::new(rpc), Box::new(repo), Rc::new(MediaManager::default()));
//    let handle = room.new_handle();
//    assert!(handle.mute_audio().is_ok());
}
//
//#[wasm_bindgen_test]
//fn mute_audio_error() {
//    let (_event_sender, event_receiver) = unbounded();
//    let mut rpc = MockRpcClient::new();
//    let mut repo = MockPeerRepository::new();
//
//    rpc.expect_subscribe()
//        .return_once(move || Box::new(event_receiver));
//    repo.expect_get_all().returning(move || {
//        let mut peer = MockPeerConnection::new();
//        peer.expect_mute_audio()
//            .returning(|| Err(WasmErr::from("error".to_string())));
//        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
//    });
//    rpc.expect_unsub().return_const(());
//
//    let room = Room::new(Rc::new(rpc), Box::new(repo), Rc::new(MediaManager::default()));
//    let handle = room.new_handle();
//    assert!(handle.mute_audio().is_err());
//}
//
//#[wasm_bindgen_test]
//fn unmute_audio_success() {
//    let (_event_sender, event_receiver) = unbounded();
//    let mut rpc = MockRpcClient::new();
//    let mut repo = MockPeerRepository::new();
//
//    rpc.expect_subscribe()
//        .return_once(move || Box::new(event_receiver));
//    repo.expect_get_all().returning(move || {
//        let mut peer = MockPeerConnection::new();
//        peer.expect_unmute_audio().returning(|| Ok(()));
//        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
//    });
//    rpc.expect_unsub().return_const(());
//
//    let room = Room::new(Rc::new(rpc), Box::new(repo), Rc::new(MediaManager::default()));
//    let handle = room.new_handle();
//    assert!(handle.unmute_audio().is_ok());
//}
//
//#[wasm_bindgen_test]
//fn unmute_audio_error() {
//    let (_event_sender, event_receiver) = unbounded();
//    let mut rpc = MockRpcClient::new();
//    let mut repo = MockPeerRepository::new();
//
//    rpc.expect_subscribe()
//        .return_once(move || Box::new(event_receiver));
//    repo.expect_get_all().returning(move || {
//        let mut peer = MockPeerConnection::new();
//        peer.expect_unmute_audio()
//            .returning(|| Err(WasmErr::from("error".to_string())));
//        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
//    });
//    rpc.expect_unsub().return_const(());
//
//    let room = Room::new(Rc::new(rpc), Box::new(repo), Rc::new(MediaManager::default()));
//    let handle = room.new_handle();
//    assert!(handle.unmute_audio().is_err());
//}
//
//#[wasm_bindgen_test]
//fn mute_video_success() {
//    let (_event_sender, event_receiver) = unbounded();
//    let mut rpc = MockRpcClient::new();
//    let mut repo = MockPeerRepository::new();
//
//    rpc.expect_subscribe()
//        .return_once(move || Box::new(event_receiver));
//    repo.expect_get_all().returning(move || {
//        let mut peer = MockPeerConnection::new();
//        peer.expect_mute_video().returning(|| Ok(()));
//        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
//    });
//    rpc.expect_unsub().return_const(());
//
//    let room = Room::new(Rc::new(rpc), Box::new(repo), Rc::new(MediaManager::default()));
//    let handle = room.new_handle();
//    assert!(handle.mute_video().is_ok());
//}
//
//#[wasm_bindgen_test]
//fn mute_video_error() {
//    let (_event_sender, event_receiver) = unbounded();
//    let mut rpc = MockRpcClient::new();
//    let mut repo = MockPeerRepository::new();
//
//    rpc.expect_subscribe()
//        .return_once(move || Box::new(event_receiver));
//    repo.expect_get_all().returning(move || {
//        let mut peer = MockPeerConnection::new();
//        peer.expect_mute_video()
//            .returning(|| Err(WasmErr::from("error".to_string())));
//        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
//    });
//    rpc.expect_unsub().return_const(());
//
//    let room = Room::new(Rc::new(rpc), Box::new(repo), Rc::new(MediaManager::default()));
//    let handle = room.new_handle();
//    assert!(handle.mute_video().is_err());
//}
//
////------ Unmute video -----
//
//#[wasm_bindgen_test]
//fn unmute_video_success() {
//    let (_event_sender, event_receiver) = unbounded();
//    let mut rpc = MockRpcClient::new();
//    let mut repo = MockPeerRepository::new();
//
//    rpc.expect_subscribe()
//        .return_once(move || Box::new(event_receiver));
//    repo.expect_get_all().returning(move || {
//        let mut peer = MockPeerConnection::new();
//        peer.expect_unmute_video().returning(|| Ok(()));
//        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
//    });
//    rpc.expect_unsub().return_const(());
//
//    let room = Room::new(Rc::new(rpc), Box::new(repo), Rc::new(MediaManager::default()));
//    let handle = room.new_handle();
//    assert!(handle.unmute_video().is_ok());
//}
//
//#[wasm_bindgen_test]
//fn unmute_video_error() {
//    let (_event_sender, event_receiver) = unbounded();
//    let mut rpc = MockRpcClient::new();
//    let mut repo = MockPeerRepository::new();
//
//    rpc.expect_subscribe()
//        .return_once(move || Box::new(event_receiver));
//    repo.expect_get_all().returning(move || {
//        let mut peer = MockPeerConnection::new();
//        peer.expect_unmute_video()
//            .returning(|| Err(WasmErr::from("error".to_string())));
//        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
//    });
//    rpc.expect_unsub().return_const(());
//
//     let room = Room::new(Rc::new(rpc), Box::new(repo), Rc::new(MediaManager::default()));
//    let handle = room.new_handle();
//    assert!(handle.unmute_video().is_err());
//}
