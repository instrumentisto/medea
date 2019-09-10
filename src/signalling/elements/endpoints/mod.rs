//! Medea endpoints implementations.

pub mod webrtc;

use derive_more::From;
use medea_grpc_proto::control::Element as RootElementProto;

#[derive(Clone, Debug, From)]
pub enum Endpoint {
    WebRtcPublishEndpoint(webrtc::WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(webrtc::WebRtcPlayEndpoint),
}

impl Into<RootElementProto> for Endpoint {
    fn into(self) -> RootElementProto {
        match self {
            Self::WebRtcPublishEndpoint(play) => play.into(),
            Self::WebRtcPlayEndpoint(publish) => publish.into(),
        }
    }
}
