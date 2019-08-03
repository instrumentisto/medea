#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use futures::{Future, Stream};
use mockers::Scenario;
use mockers_derive::mocked;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

use medea_client_api_proto::{Command, Event};

use futures::sync::mpsc::unbounded;
use jason::{
    api::Room,
    media::MediaManager,
    peer::{PeerConnection, PeerId},
};

wasm_bindgen_test_configure!(run_in_browser);

#[mocked(RpcClientMock, extern, module = "::jason::rpc")]
trait RpcClient {
    fn subscribe(&self) -> Box<dyn Stream<Item = Event, Error = ()>>;
    fn unsub(&self);
    fn send_command(&self, command: Command);
}

#[mocked(PeerRepositoryMock, extern, module = "::jason::peer")]
pub trait PeerRepository {
    /// Stores [`PeerConnection`] in repository.
    fn insert(
        &mut self,
        id: PeerId,
        peer: Rc<PeerConnection>,
    ) -> Option<Rc<PeerConnection>>;

    /// Returns [`PeerConnection`] stored in repository by its ID.
    fn get(&self, id: PeerId) -> Option<Rc<PeerConnection>>;

    /// Removes [`PeerConnection`] stored in repository by its ID.
    fn remove(&mut self, id: PeerId);

    /// Returns all [`PeerConnection`]s stored in repository.
    fn get_all(&self) -> Vec<Rc<PeerConnection>>;
}

#[wasm_bindgen_test(async)]
fn mute_audio() -> impl Future<Item = (), Error = JsValue> {
    let media_manager = Rc::new(MediaManager::default());
    let _event = Event::PeerCreated {
        peer_id: 1,
        ice_servers: vec![],
        sdp_offer: None,
        tracks: vec![],
    };
    let (_event_sender, event_receiver) = unbounded();
    // let stream = Box::new(once::<_, ()>(Ok(event)));
    // let stream = Box::new(futures::done(Ok(event)).into_stream());

    let scenario = Scenario::new();
    let (rpc, rpc_handle) = scenario.create_mock::<RpcClientMock>();
    let (peers, peers_handle) = scenario.create_mock::<PeerRepositoryMock>();
    scenario
        .expect(rpc_handle.subscribe().and_return(Box::new(event_receiver)));
    scenario.expect(peers_handle.get_all().and_return(vec![]));
    // scenario.expect(rpc_handle.subscribe().and_return(stream));
    // scenario.expect(rpc_handle.send_command(ANY).and_return(()));
    scenario.expect(rpc_handle.unsub().and_return(()));

    let room = Room::new(Rc::new(rpc), Box::new(peers), &media_manager);
    let handle = room.new_handle();
    assert!(handle.mute_audio().is_ok());

    futures::done(Ok(()))
}
