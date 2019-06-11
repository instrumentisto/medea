use super::member::Participant;
use crate::api::control::{endpoint::SrcUri, MemberId};
use std::cell::RefCell;
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub String);

#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpointInner {
    pub src: SrcUri,
    pub publisher: Weak<WebRtcPublishEndpoint>,
    pub owner: Weak<Participant>,
    pub is_connected: bool,
}

impl WebRtcPlayEndpointInner {
    pub fn src(&self) -> SrcUri {
        self.src.clone()
    }

    pub fn owner(&self) -> Weak<Participant> {
        Weak::clone(&self.owner)
    }

    pub fn publisher(&self) -> Weak<WebRtcPublishEndpoint> {
        self.publisher.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    pub fn set_is_connected(&mut self, value: bool) {
        self.is_connected = value;
    }
}

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
            is_connected: false,
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

    pub fn connected(&self) {
        self.0.lock().unwrap().borrow_mut().set_is_connected(true);
    }
}

#[derive(Debug, Clone)]
pub enum P2pMode {
    Always,
}

#[derive(Debug, Clone)]
pub struct WebRtcPublishEndpointInner {
    pub p2p: P2pMode,
    pub receivers: Vec<Weak<WebRtcPlayEndpoint>>,
    pub owner: Weak<Participant>,
}

impl WebRtcPublishEndpointInner {
    pub fn add_receiver(&mut self, receiver: Weak<WebRtcPlayEndpoint>) {
        self.receivers.push(receiver);
    }

    pub fn receivers(&self) -> Vec<Weak<WebRtcPlayEndpoint>> {
        self.receivers.clone()
    }

    pub fn owner(&self) -> Weak<Participant> {
        Weak::clone(&self.owner)
    }
}

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
}
