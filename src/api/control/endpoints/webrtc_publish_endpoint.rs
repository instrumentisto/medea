//! `WebRtcPublishEndpoint` implementation.

use std::convert::TryFrom;

use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;

use crate::api::control::{
    grpc::protos::control::{
        WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
        WebRtcPublishEndpoint_P2P as WebRtcPublishEndpointP2pProto,
    },
    TryFromProtobufError,
};

macro_attr! {
    /// ID of [`WebRtcPublishEndpoint`].
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
    pub struct WebRtcPublishId(pub String);
}

/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
#[derive(Clone, Deserialize, Debug)]
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,
    Never,
    IfPossible,
}

impl From<WebRtcPublishEndpointP2pProto> for P2pMode {
    fn from(value: WebRtcPublishEndpointP2pProto) -> Self {
        match value {
            WebRtcPublishEndpointP2pProto::ALWAYS => P2pMode::Always,
            WebRtcPublishEndpointP2pProto::IF_POSSIBLE => P2pMode::IfPossible,
            WebRtcPublishEndpointP2pProto::NEVER => P2pMode::Never,
        }
    }
}

impl Into<WebRtcPublishEndpointP2pProto> for P2pMode {
    fn into(self) -> WebRtcPublishEndpointP2pProto {
        match self {
            P2pMode::Always => WebRtcPublishEndpointP2pProto::ALWAYS,
            P2pMode::IfPossible => WebRtcPublishEndpointP2pProto::IF_POSSIBLE,
            P2pMode::Never => WebRtcPublishEndpointP2pProto::NEVER,
        }
    }
}

/// Media element which is able to publish media data for another client via
/// WebRTC.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode.
    pub p2p: P2pMode,
}

impl TryFrom<&WebRtcPublishEndpointProto> for WebRtcPublishEndpoint {
    type Error = TryFromProtobufError;

    fn try_from(
        value: &WebRtcPublishEndpointProto,
    ) -> Result<Self, Self::Error> {
        if value.has_p2p() {
            Ok(Self {
                p2p: P2pMode::from(value.get_p2p()),
            })
        } else {
            Err(TryFromProtobufError::P2pModeNotFound)
        }
    }
}
