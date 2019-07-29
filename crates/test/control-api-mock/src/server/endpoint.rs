//! Endpoint related methods and entities.

use actix_web::{
    web::{Data, Json, Path},
    HttpResponse,
};
use futures::Future;
use medea_grpc_proto::control::{
    Member_Element as MemberElementProto,
    WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
    WebRtcPublishEndpoint_P2P as P2pModeProto,
};
use serde::{Deserialize, Serialize};

use crate::{
    client::EndpointUri,
    prelude::*,
    server::{Context, Response, SingleGetResponse},
};

/// Path to `Endpoint` in REST API.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize)]
pub struct EndpointPath {
    pub room_id: String,
    pub member_id: String,
    pub endpoint_id: String,
}

/// `DELETE /{room_id}/{member_id}/{endpoint_id}`
///  Delete `Endpoint`.
///
/// _For batch delete use `DELETE /`._
#[allow(clippy::needless_pass_by_value)]
pub fn delete(
    path: Path<EndpointPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .delete_single(EndpointUri::from(path))
        .map_err(|e| error!("{:?}", e))
        .map(|r| Response::from(r).into())
}

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
            P2pMode::Always => P2pModeProto::ALWAYS,
            P2pMode::IfPossible => P2pModeProto::IF_POSSIBLE,
            P2pMode::Never => P2pModeProto::NEVER,
        }
    }
}

impl From<P2pModeProto> for P2pMode {
    fn from(proto: P2pModeProto) -> Self {
        match proto {
            P2pModeProto::ALWAYS => P2pMode::Always,
            P2pModeProto::IF_POSSIBLE => P2pMode::IfPossible,
            P2pModeProto::NEVER => P2pMode::Never,
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

/// Control API's `WebRtcPlayEndpoint` representation.
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

/// Some `Endpoint` representation.
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

impl From<MemberElementProto> for Endpoint {
    fn from(mut proto: MemberElementProto) -> Self {
        if proto.has_webrtc_play() {
            Endpoint::WebRtcPlayEndpoint {
                spec: proto.take_webrtc_play().into(),
            }
        } else if proto.has_webrtc_pub() {
            Endpoint::WebRtcPublishEndpoint {
                spec: proto.take_webrtc_pub().into(),
            }
        } else {
            unimplemented!()
        }
    }
}

/// `POST /{room_id}/{member_id}/{endpoint_id}`
///
/// Create new `Endpoint`.
#[allow(clippy::needless_pass_by_value)]
pub fn create(
    path: Path<EndpointPath>,
    state: Data<Context>,
    data: Json<Endpoint>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .create_endpoint(&path.into(), data.0)
        .map_err(|e| error!("{:?}", e))
        .map(|r| Response::from(r).into())
}

/// `GET /{room_id}/{member_id}/{endpoint_id}`
///
/// Get single `Endpoint`.
///
/// For batch get use `GET /`.
#[allow(clippy::needless_pass_by_value)]
pub fn get(
    path: Path<EndpointPath>,
    state: Data<Context>,
) -> impl Future<Item = HttpResponse, Error = ()> {
    state
        .client
        .get_single(EndpointUri::from(path))
        .map_err(|e| error!("{:?}", e))
        .map(|r| SingleGetResponse::from(r).into())
}
