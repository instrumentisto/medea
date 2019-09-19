//! [Medea] endpoints implementations.
//!
//! [Medea]: https://github.com/instrumentisto/medea

pub mod webrtc;

use derive_more::From;
use medea_control_api_proto::grpc::control_api::Element as RootElementProto;

/// Enum which can store all kinds of [medea] endpoints.
///
/// [medea]: https://github.com/instrumentisto/medea
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
