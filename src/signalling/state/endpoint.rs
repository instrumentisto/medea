use std::{
    cell::RefCell,
    sync::{Mutex, Weak},
};

use crate::api::control::endpoint::{P2pMode, SrcUri};
use crate::media::PeerId;

use super::participant::Participant;
use hashbrown::HashSet;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub String);

#[derive(Debug, Clone)]
struct WebRtcPlayEndpointInner {
    src: SrcUri,
    publisher: Weak<WebRtcPublishEndpoint>,
    owner: Weak<Participant>,
    peer_id: Option<PeerId>,
}

impl WebRtcPlayEndpointInner {
    fn src(&self) -> SrcUri {
        self.src.clone()
    }

    fn owner(&self) -> Weak<Participant> {
        Weak::clone(&self.owner)
    }

    fn publisher(&self) -> Weak<WebRtcPublishEndpoint> {
        self.publisher.clone()
    }

    fn is_connected(&self) -> bool {
        self.peer_id.is_some()
    }

    fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = Some(peer_id)
    }

    fn peer_id(&self) -> Option<PeerId> {
        self.peer_id.clone()
    }

    fn reset(&mut self) {
        self.peer_id = None
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct WebRtcPlayEndpoint(Mutex<RefCell<WebRtcPlayEndpointInner>>);

impl WebRtcPlayEndpoint {
    pub fn new(
        src: SrcUri,
        publisher: Weak<WebRtcPublishEndpoint>,
        owner: Weak<Participant>,
    ) -> Self {
        Self(Mutex::new(RefCell::new(WebRtcPlayEndpointInner {
            src,
            publisher,
            owner,
            peer_id: None,
        })))
    }

    pub fn src(&self) -> SrcUri {
        self.0.lock().unwrap().borrow().src()
    }

    pub fn owner(&self) -> Weak<Participant> {
        self.0.lock().unwrap().borrow().owner()
    }

    pub fn publisher(&self) -> Weak<WebRtcPublishEndpoint> {
        self.0.lock().unwrap().borrow().publisher()
    }

    pub fn is_connected(&self) -> bool {
        self.0.lock().unwrap().borrow().is_connected()
    }

    pub fn connect(&self, peer_id: PeerId) {
        self.0.lock().unwrap().borrow_mut().set_peer_id(peer_id);
    }

    pub fn peer_id(&self) -> Option<PeerId> {
        self.0.lock().unwrap().borrow().peer_id()
    }

    pub fn reset(&self) {
        self.0.lock().unwrap().borrow_mut().reset()
    }
}

#[derive(Debug, Clone)]
struct WebRtcPublishEndpointInner {
    p2p: P2pMode,
    receivers: Vec<Weak<WebRtcPlayEndpoint>>,
    owner: Weak<Participant>,
    peer_ids: HashSet<PeerId>,
}

impl WebRtcPublishEndpointInner {
    fn add_receiver(&mut self, receiver: Weak<WebRtcPlayEndpoint>) {
        self.receivers.push(receiver);
    }

    fn receivers(&self) -> Vec<Weak<WebRtcPlayEndpoint>> {
        self.receivers.clone()
    }

    fn owner(&self) -> Weak<Participant> {
        Weak::clone(&self.owner)
    }

    fn add_peer_id(&mut self, peer_id: PeerId) {
        self.peer_ids.insert(peer_id);
    }

    fn peer_ids(&self) -> HashSet<PeerId> {
        self.peer_ids.clone()
    }

    pub fn reset(&mut self) {
        self.peer_ids = HashSet::new()
    }

    pub fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.peer_ids.remove(peer_id);
    }

    pub fn remove_peer_ids(&mut self, peer_ids: &Vec<PeerId>) {
        for peer_id in peer_ids {
            self.remove_peer_id(peer_id)
        }
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct WebRtcPublishEndpoint(Mutex<RefCell<WebRtcPublishEndpointInner>>);

impl WebRtcPublishEndpoint {
    pub fn new(
        p2p: P2pMode,
        receivers: Vec<Weak<WebRtcPlayEndpoint>>,
        owner: Weak<Participant>,
    ) -> Self {
        Self(Mutex::new(RefCell::new(WebRtcPublishEndpointInner {
            p2p,
            receivers,
            owner,
            peer_ids: HashSet::new(),
        })))
    }

    pub fn add_receiver(&self, receiver: Weak<WebRtcPlayEndpoint>) {
        self.0.lock().unwrap().borrow_mut().add_receiver(receiver)
    }

    pub fn receivers(&self) -> Vec<Weak<WebRtcPlayEndpoint>> {
        self.0.lock().unwrap().borrow().receivers()
    }

    pub fn owner(&self) -> Weak<Participant> {
        self.0.lock().unwrap().borrow().owner()
    }

    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0.lock().unwrap().borrow_mut().add_peer_id(peer_id)
    }

    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.lock().unwrap().borrow().peer_ids()
    }

    pub fn reset(&self) {
        self.0.lock().unwrap().borrow_mut().reset()
    }

    pub fn remove_peer_id(&self, peer_id: &PeerId) {
        self.0.lock().unwrap().borrow_mut().remove_peer_id(peer_id)
    }

    pub fn remove_peer_ids(&self, peer_ids: &Vec<PeerId>) {
        self.0
            .lock()
            .unwrap()
            .borrow_mut()
            .remove_peer_ids(peer_ids)
    }
}
