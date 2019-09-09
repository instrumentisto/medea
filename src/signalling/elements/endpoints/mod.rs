//! Medea endpoints implementations.

pub mod webrtc;

use medea_grpc_proto::control::Element as RootElementProto;

pub enum Endpoint {
    WebRtcPublishEndpoint(webrtc::WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(webrtc::WebRtcPlayEndpoint),
}

// TODO: maybe better?
impl Into<RootElementProto> for Endpoint {
    fn into(self) -> RootElementProto {
        match self {
            Self::WebRtcPublishEndpoint(play) => play.into(),
            Self::WebRtcPlayEndpoint(publish) => publish.into(),
        }
    }
}
