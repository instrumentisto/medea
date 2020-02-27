//! [Medea] endpoints implementations.
//!
//! [Medea]: https://github.com/instrumentisto/medea

pub mod webrtc;

use derive_more::From;
use medea_control_api_proto::grpc::api as proto;

/// Enum which can store all kinds of [Medea] endpoints.
///
/// [Medea]: https://github.com/instrumentisto/medea
#[derive(Clone, Debug, From)]
pub enum Endpoint {
    WebRtcPublishEndpoint(webrtc::WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(webrtc::WebRtcPlayEndpoint),
}

impl Into<proto::Element> for Endpoint {
    fn into(self) -> proto::Element {
        match self {
            Self::WebRtcPublishEndpoint(play) => play.into(),
            Self::WebRtcPlayEndpoint(publish) => publish.into(),
        }
    }
}

#[derive(Clone, Debug, From)]
pub enum WeakEndpoint {
    WebRtcPublishEndpoint(webrtc::publish_endpoint::WeakWebRtcPublishEndpoint),
    WebRtcPlayEndpoint(webrtc::play_endpoint::WeakWebRtcPlayEndpoint),
}

impl WeakEndpoint {
    pub fn upgrade(&self) -> Option<Endpoint> {
        match self {
            WeakEndpoint::WebRtcPublishEndpoint(publish_endpoint) => {
                publish_endpoint.safe_upgrade().map(|e| e.into())
            }
            WeakEndpoint::WebRtcPlayEndpoint(play_endpoint) => {
                play_endpoint.safe_upgrade().map(|e| e.into())
            }
        }
    }
}
