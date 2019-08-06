#![cfg(target_arch = "wasm32")]

use std::{collections::HashMap, rc::Rc};

use futures::{sync::mpsc::unbounded, Future, Stream};
use mockall::{predicate::*, *};
use wasm_bindgen_test::*;

use medea_client_api_proto::{Command, Event, Track};

use jason::{
    api::Room,
    media::MediaManager,
    peer::{PeerConnection, PeerId, PeerRepository},
    rpc::RpcClient,
    utils::WasmErr,
};

wasm_bindgen_test_configure!(run_in_browser);

mock! {
    RpcClient {}
    trait RpcClient {
        fn subscribe(&self) -> Box<dyn Stream<Item = Event, Error = ()>>;
        fn unsub(&self);
        fn send_command(&self, command: Command);
    }
}

mock! {
    PeerRepository {}
    pub trait PeerRepository {
        fn insert(
            &mut self,
            id: PeerId,
            peer: Rc<PeerConnection>,
        ) -> Option<Rc<PeerConnection>>;
        fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>>;
        fn remove(&mut self, id: PeerId);
        fn get_all(&self) -> Vec<Rc<PeerConnection>>;
    }
}

mock! {
    PeerConnection {}
    pub trait PeerConnection {
        fn mute_audio(&self) -> Result<(), WasmErr>;
        fn enabled_audio(&self) -> Result<bool, WasmErr>;
        fn unmute_audio(&self) -> Result<(), WasmErr>;
        fn mute_video(&self) -> Result<(), WasmErr>;
        fn enabled_video(&self) -> Result<bool, WasmErr>;
        fn unmute_video(&self) -> Result<(), WasmErr>;
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
    let media_manager = Rc::new(MediaManager::default());
    let (_event_sender, event_receiver) = unbounded();
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(move || {
        let mut peer = MockPeerConnection::new();
        peer.expect_mute_audio().returning(|| Ok(()));
        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
    });
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), Box::new(repo), &media_manager);
    let handle = room.new_handle();
    assert!(handle.mute_audio().is_ok());
}

#[wasm_bindgen_test]
fn mute_audio_error() {
    let media_manager = Rc::new(MediaManager::default());
    let (_event_sender, event_receiver) = unbounded();
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(move || {
        let mut peer = MockPeerConnection::new();
        peer.expect_mute_audio()
            .returning(|| Err(WasmErr::from("error".to_string())));
        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
    });
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), Box::new(repo), &media_manager);
    let handle = room.new_handle();
    assert!(handle.mute_audio().is_err());
}

#[wasm_bindgen_test]
fn unmute_audio_success() {
    let (_event_sender, event_receiver) = unbounded();
    let media_manager = Rc::new(MediaManager::default());
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(move || {
        let mut peer = MockPeerConnection::new();
        peer.expect_unmute_audio().returning(|| Ok(()));
        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
    });
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), Box::new(repo), &media_manager);
    let handle = room.new_handle();
    assert!(handle.unmute_audio().is_ok());
}

#[wasm_bindgen_test]
fn unmute_audio_error() {
    let media_manager = Rc::new(MediaManager::default());
    let (_event_sender, event_receiver) = unbounded();
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(move || {
        let mut peer = MockPeerConnection::new();
        peer.expect_unmute_audio()
            .returning(|| Err(WasmErr::from("error".to_string())));
        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
    });
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), Box::new(repo), &media_manager);
    let handle = room.new_handle();
    assert!(handle.unmute_audio().is_err());
}

#[wasm_bindgen_test]
fn mute_video_success() {
    let media_manager = Rc::new(MediaManager::default());
    let (_event_sender, event_receiver) = unbounded();
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(move || {
        let mut peer = MockPeerConnection::new();
        peer.expect_mute_video().returning(|| Ok(()));
        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
    });
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), Box::new(repo), &media_manager);
    let handle = room.new_handle();
    assert!(handle.mute_video().is_ok());
}

#[wasm_bindgen_test]
fn mute_video_error() {
    let media_manager = Rc::new(MediaManager::default());
    let (_event_sender, event_receiver) = unbounded();
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(move || {
        let mut peer = MockPeerConnection::new();
        peer.expect_mute_video()
            .returning(|| Err(WasmErr::from("error".to_string())));
        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
    });
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), Box::new(repo), &media_manager);
    let handle = room.new_handle();
    assert!(handle.mute_video().is_err());
}

//------ Unmute video -----

#[wasm_bindgen_test]
fn unmute_video_success() {
    let media_manager = Rc::new(MediaManager::default());
    let (_event_sender, event_receiver) = unbounded();
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(move || {
        let mut peer = MockPeerConnection::new();
        peer.expect_unmute_video().returning(|| Ok(()));
        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
    });
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), Box::new(repo), &media_manager);
    let handle = room.new_handle();
    assert!(handle.unmute_video().is_ok());
}

#[wasm_bindgen_test]
fn unmute_video_error() {
    let media_manager = Rc::new(MediaManager::default());
    let (_event_sender, event_receiver) = unbounded();
    let mut rpc = MockRpcClient::new();
    let mut repo = MockPeerRepository::new();

    rpc.expect_subscribe()
        .return_once(move || Box::new(event_receiver));
    repo.expect_get_all().returning(move || {
        let mut peer = MockPeerConnection::new();
        peer.expect_unmute_video()
            .returning(|| Err(WasmErr::from("error".to_string())));
        vec![Rc::new(peer) as Rc<dyn PeerConnection>]
    });
    rpc.expect_unsub().return_const(());

    let room = Room::new(Rc::new(rpc), Box::new(repo), &media_manager);
    let handle = room.new_handle();
    assert!(handle.unmute_video().is_err());
}
