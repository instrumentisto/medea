use super::member::Participant;
use crate::api::control::{endpoint::SrcUri, MemberId};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub String);

#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpointInner {
    pub src: SrcUri,
    pub publisher: WebRtcPublishEndpoint,
    pub owner_id: MemberId,
    pub is_connected: bool,
}

impl WebRtcPlayEndpointInner {
    pub fn src(&self) -> SrcUri {
        self.src.clone()
    }

    pub fn owner_id(&self) -> MemberId {
        self.owner_id.clone()
    }

    pub fn publisher(&self) -> WebRtcPublishEndpoint {
        self.publisher.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    pub fn set_is_connected(&mut self, value: bool) {
        self.is_connected = value;
    }
}

#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpoint(Arc<Mutex<RefCell<WebRtcPlayEndpointInner>>>);

impl WebRtcPlayEndpoint {
    pub fn new(
        src: SrcUri,
        publisher: WebRtcPublishEndpoint,
        owner_id: MemberId,
    ) -> Self {
        Self(Arc::new(Mutex::new(RefCell::new(
            WebRtcPlayEndpointInner {
                src,
                publisher,
                owner_id,
                is_connected: false,
            },
        ))))
    }

    pub fn src(&self) -> SrcUri {
        self.0.lock().unwrap().borrow().src()
    }

    pub fn owner_id(&self) -> MemberId {
        self.0.lock().unwrap().borrow().owner_id()
    }

    pub fn publisher(&self) -> WebRtcPublishEndpoint {
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
    pub receivers: Vec<WebRtcPlayEndpoint>,
    pub owner_id: MemberId,
}

impl WebRtcPublishEndpointInner {
    pub fn add_receiver(&mut self, receiver: WebRtcPlayEndpoint) {
        self.receivers.push(receiver);
    }

    pub fn receivers(&self) -> Vec<WebRtcPlayEndpoint> {
        self.receivers.clone()
    }

    pub fn owner_id(&self) -> MemberId {
        self.owner_id.clone()
    }
}

#[derive(Debug, Clone)]
pub struct WebRtcPublishEndpoint(
    Arc<Mutex<RefCell<WebRtcPublishEndpointInner>>>,
);

impl WebRtcPublishEndpoint {
    pub fn new(
        p2p: P2pMode,
        receivers: Vec<WebRtcPlayEndpoint>,
        owner_id: MemberId,
    ) -> Self {
        Self(Arc::new(Mutex::new(RefCell::new(
            WebRtcPublishEndpointInner {
                p2p,
                receivers,
                owner_id,
            },
        ))))
    }

    pub fn add_receiver(&self, receiver: WebRtcPlayEndpoint) {
        self.0.lock().unwrap().borrow_mut().add_receiver(receiver)
    }

    pub fn receivers(&self) -> Vec<WebRtcPlayEndpoint> {
        self.0.lock().unwrap().borrow().receivers()
    }

    pub fn owner_id(&self) -> MemberId {
        self.0.lock().unwrap().borrow().owner_id()
    }
}
