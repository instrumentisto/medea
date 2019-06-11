use super::member::Participant;
use crate::api::control::endpoint::SrcUri;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub String);

#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpointInner {
    pub src: SrcUri,
    pub publisher: WebRtcPublishEndpoint,
    pub owner: Participant,
}

impl WebRtcPlayEndpointInner {
    pub fn src(&self) -> SrcUri {
        self.src.clone()
    }

    pub fn owner(&self) -> Participant {
        self.owner.clone()
    }

    pub fn publisher(&self) -> WebRtcPublishEndpoint {
        self.publisher.clone()
    }
}

#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpoint(Arc<Mutex<RefCell<WebRtcPlayEndpointInner>>>);

impl WebRtcPlayEndpoint {
    pub fn new(
        src: SrcUri,
        publisher: WebRtcPublishEndpoint,
        owner: Participant,
    ) -> Self {
        Self(Arc::new(Mutex::new(RefCell::new(
            WebRtcPlayEndpointInner {
                src,
                publisher,
                owner,
            },
        ))))
    }

    pub fn src(&self) -> SrcUri {
        self.0.lock().unwrap().borrow().src()
    }

    pub fn owner(&self) -> Participant {
        self.0.lock().unwrap().borrow().owner()
    }

    pub fn publisher(&self) -> WebRtcPublishEndpoint {
        self.0.lock().unwrap().borrow().publisher()
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
    pub owner: Participant,
}

impl WebRtcPublishEndpointInner {
    pub fn add_receiver(&mut self, receiver: WebRtcPlayEndpoint) {
        self.receivers.push(receiver);
    }

    pub fn receivers(&self) -> Vec<WebRtcPlayEndpoint> {
        self.receivers.clone()
    }

    pub fn owner(&self) -> Participant {
        self.owner.clone()
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
        owner: Participant,
    ) -> Self {
        Self(Arc::new(Mutex::new(RefCell::new(
            WebRtcPublishEndpointInner {
                p2p,
                receivers,
                owner,
            },
        ))))
    }

    pub fn add_receiver(&self, receiver: WebRtcPlayEndpoint) {
        self.0.lock().unwrap().borrow_mut().add_receiver(receiver)
    }

    pub fn receivers(&self) -> Vec<WebRtcPlayEndpoint> {
        self.0.lock().unwrap().borrow().receivers()
    }

    pub fn owner(&self) -> Participant {
        self.0.lock().unwrap().borrow().owner()
    }
}
