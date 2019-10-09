//! `Endpoint` related methods and entities.

use medea_control_api_proto::grpc::control_api::{
    Member_Element as MemberElementProto,
    WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
    WebRtcPublishEndpoint_P2P as P2pModeProto,
};
use serde::{Deserialize, Serialize};

/// P2p mode of [`WebRtcPublishEndpoint`].
#[derive(Serialize, Deserialize, Debug)]
pub enum P2pMode {
    Always,
    Never,
    IfPossible,
}

impl Into<P2pModeProto> for P2pMode {
    fn into(self) -> P2pModeProto {
        match self {
            Self::Always => P2pModeProto::ALWAYS,
            Self::IfPossible => P2pModeProto::IF_POSSIBLE,
            Self::Never => P2pModeProto::NEVER,
        }
    }
}

impl From<P2pModeProto> for P2pMode {
    fn from(proto: P2pModeProto) -> Self {
        match proto {
            P2pModeProto::ALWAYS => Self::Always,
            P2pModeProto::IF_POSSIBLE => Self::IfPossible,
            P2pModeProto::NEVER => Self::Never,
        }
    }
}

/// Control API's `WebRtcPublishEndpoint` representation.
#[allow(clippy::module_name_repetitions)]
#[derive(Serialize, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
    /// Mode of connection for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,
}

impl Into<WebRtcPublishEndpointProto> for WebRtcPublishEndpoint {
    fn into(self) -> WebRtcPublishEndpointProto {
        let mut proto = WebRtcPublishEndpointProto::new();
        proto.set_p2p(self.p2p.into());
        proto
    }
}

impl From<WebRtcPublishEndpointProto> for WebRtcPublishEndpoint {
    fn from(proto: WebRtcPublishEndpointProto) -> Self {
        Self {
            p2p: proto.get_p2p().into(),
        }
    }
}

/// Control API's `WebRtcPlayEndpoint` element representation.
#[allow(clippy::module_name_repetitions)]
#[derive(Serialize, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint {
    /// URI in format `local://{room_id}/{member_id}/{endpoint_id}` pointing to
    /// [`WebRtcPublishEndpoint`] which this [`WebRtcPlayEndpoint`] plays.
    src: String,
}

impl Into<WebRtcPlayEndpointProto> for WebRtcPlayEndpoint {
    fn into(self) -> WebRtcPlayEndpointProto {
        let mut proto = WebRtcPlayEndpointProto::new();
        proto.set_src(self.src);
        proto
    }
}

impl From<WebRtcPlayEndpointProto> for WebRtcPlayEndpoint {
    fn from(mut proto: WebRtcPlayEndpointProto) -> Self {
        Self {
            src: proto.take_src(),
        }
    }
}

/// `Endpoint` element representation.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind")]
pub enum Endpoint {
    WebRtcPublishEndpoint { spec: WebRtcPublishEndpoint },
    WebRtcPlayEndpoint { spec: WebRtcPlayEndpoint },
}

impl Into<MemberElementProto> for Endpoint {
    fn into(self) -> MemberElementProto {
        let mut proto = MemberElementProto::new();
        match self {
            Self::WebRtcPlayEndpoint { spec } => {
                proto.set_webrtc_play(spec.into())
            }
            Self::WebRtcPublishEndpoint { spec } => {
                proto.set_webrtc_pub(spec.into())
            }
        }
        proto
    }
}

impl From<MemberElementProto> for Endpoint {
    fn from(mut proto: MemberElementProto) -> Self {
        if proto.has_webrtc_play() {
            Self::WebRtcPlayEndpoint {
                spec: proto.take_webrtc_play().into(),
            }
        } else if proto.has_webrtc_pub() {
            Self::WebRtcPublishEndpoint {
                spec: proto.take_webrtc_pub().into(),
            }
        } else {
            unimplemented!()
        }
    }
}
