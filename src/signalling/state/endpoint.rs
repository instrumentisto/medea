use super::member::Participant;
use crate::api::control::endpoint::SrcUri;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Id(String);

#[derive(Debug, Clone)]
pub enum P2pMode {
    Always,
}

#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpoint {
    pub src: SrcUri,
    pub publisher: Arc<WebRtcPublishEndpoint>,
    pub participant: Participant,
}

#[derive(Debug, Clone)]
pub struct WebRtcPublishEndpoint {
    pub p2p: P2pMode,
    pub receivers: Vec<Arc<WebRtcPlayEndpoint>>,
    pub participant: Participant,
}
