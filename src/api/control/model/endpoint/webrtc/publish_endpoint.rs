use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize; // TODO: temp

use crate::api::grpc::protos::control::WebRtcPublishEndpoint_P2P;

macro_attr! {
    /// ID of [`Room`].
    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        Hash,
        PartialEq,
        NewtypeFrom!,
        NewtypeDisplay!,
    )]
    pub struct Id(pub String);
}

pub use Id as WebRtcPublishId;

pub trait WebRtcPublishEndpoint {
    fn p2p(&self) -> P2pMode;
}

/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
#[derive(Clone, Deserialize, Debug)]
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,
    Never,
    IfPossible,
}

impl From<WebRtcPublishEndpoint_P2P> for P2pMode {
    fn from(from: WebRtcPublishEndpoint_P2P) -> Self {
        match from {
            WebRtcPublishEndpoint_P2P::ALWAYS => P2pMode::Always,
            WebRtcPublishEndpoint_P2P::IF_POSSIBLE => P2pMode::IfPossible,
            WebRtcPublishEndpoint_P2P::NEVER => P2pMode::Never,
        }
    }
}
