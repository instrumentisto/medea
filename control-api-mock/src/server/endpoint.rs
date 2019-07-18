use actix_web::{
    web::{Data, Json, Path},
    HttpResponse,
};
use futures::Future;
use medea::api::control::grpc::protos::control::{
    CreateRequest as CreateRequestProto, Member_Element as MemberElementProto,
    WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
    WebRtcPublishEndpoint_P2P as P2pModeProto,
};
use serde::{Deserialize, Serialize};

use crate::{
    prelude::*,
    server::{Context, Response},
};
use medea::api::control::TryFromProtobufError::P2pModeNotFound;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize)]
pub struct EndpointPath {
    pub room_id: String,
    pub member_id: String,
    pub endpoint_id: String,
}

#[allow(clippy::needless_pass_by_value)]
pub fn delete(
    path: Path<EndpointPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_endpoint(path.into())
        .map(|r| Response::from(r).into())
        .map_err(|e| error!("{:?}", e))
}

#[derive(Serialize, Deserialize, Debug)]
pub enum P2pMode {
    Always,
    Never,
    IfPossible,
}

impl Into<P2pModeProto> for P2pMode {
    fn into(self) -> P2pModeProto {
        match self {
            P2pMode::Always => P2pModeProto::ALWAYS,
            P2pMode::IfPossible => P2pModeProto::IF_POSSIBLE,
            P2pMode::Never => P2pModeProto::NEVER,
        }
    }
}

impl From<P2pModeProto> for P2pMode {
    fn from(proto: P2pModeProto) -> P2pMode {
        match proto {
            P2pModeProto::ALWAYS => P2pMode::Always,
            P2pModeProto::IF_POSSIBLE => P2pMode::IfPossible,
            P2pModeProto::NEVER => P2pMode::Never,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
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

#[derive(Serialize, Deserialize, Debug)]
pub struct WebRtcPlayEndpoint {
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
            Endpoint::WebRtcPlayEndpoint { spec } => {
                proto.set_webrtc_play(spec.into())
            }
            Endpoint::WebRtcPublishEndpoint { spec } => {
                proto.set_webrtc_pub(spec.into())
            }
        }
        proto
    }
}

#[allow(clippy::needless_pass_by_value)]
pub fn create(
    path: Path<EndpointPath>,
    state: Data<Context>,
    data: Json<Endpoint>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .create_endpoint(path.into(), data.0)
        .map(|r| Response::from(r).into())
        .map_err(|e| error!("{:?}", e))
}
