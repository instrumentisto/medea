//! Implementation of client for medea's gRPC control API.

use std::{fmt, sync::Arc};

use actix_web::web::Path;
use futures::Future;
use grpcio::{ChannelBuilder, EnvBuilder, Error};
use medea_control_api_proto::grpc::control_api::{
    control::{CreateRequest, GetResponse, IdRequest, Response},
    control_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

use crate::server::{
    endpoint::{Endpoint, EndpointPath},
    member::{Member, MemberPath},
    room::{Room, RoomPath},
};

/// Uri to `Room` element.
#[derive(Clone, Debug)]
pub struct RoomUri {
    room_id: String,
}

impl From<Path<RoomPath>> for RoomUri {
    fn from(path: Path<RoomPath>) -> Self {
        Self {
            room_id: path.into_inner().room_id,
        }
    }
}

impl fmt::Display for RoomUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local://{}", self.room_id)
    }
}

/// Uri to `Member` element.
#[derive(Clone, Debug)]
pub struct MemberUri {
    room_id: String,
    member_id: String,
}

impl From<Path<MemberPath>> for MemberUri {
    fn from(path: Path<MemberPath>) -> Self {
        let path = path.into_inner();
        Self {
            room_id: path.room_id,
            member_id: path.member_id,
        }
    }
}

impl fmt::Display for MemberUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local://{}/{}", self.room_id, self.member_id)
    }
}

/// Uri to `Endpoint` element.
#[derive(Clone, Debug)]
pub struct EndpointUri {
    room_id: String,
    member_id: String,
    endpoint_id: String,
}

impl From<Path<EndpointPath>> for EndpointUri {
    fn from(path: Path<EndpointPath>) -> Self {
        let path = path.into_inner();
        Self {
            room_id: path.room_id,
            member_id: path.member_id,
            endpoint_id: path.endpoint_id,
        }
    }
}

impl fmt::Display for EndpointUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "local://{}/{}/{}",
            self.room_id, self.member_id, self.endpoint_id
        )
    }
}

/// Create new [`IdRequest`] with provided IDs.
fn id_request(ids: Vec<String>) -> IdRequest {
    let mut req = IdRequest::new();
    let ids = RepeatedField::from(ids);
    req.set_id(ids);
    req
}

/// Client for medea's control API.
#[allow(clippy::module_name_repetitions)]
pub struct ControlClient {
    grpc_client: ControlApiClient,
}

impl ControlClient {
    /// Create new client for medea's control API.
    ///
    /// __Note that call of this function is not check availability of control
    /// API's gRPC server. He's availability check only on some method call.__
    pub fn new(medea_addr: &str) -> Self {
        Self {
            grpc_client: get_grpc_client(medea_addr),
        }
    }

    /// Create `Room` element.
    pub fn create_room(
        &self,
        uri: &RoomUri,
        room: Room,
    ) -> impl Future<Item = Response, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_room(room.into());
        req.set_id(uri.to_string());

        self.grpc_client.create_async(&req).unwrap()
    }

    /// Create `Member` element.
    pub fn create_member(
        &self,
        uri: &MemberUri,
        member: Member,
    ) -> impl Future<Item = Response, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_member(member.into());
        req.set_id(uri.to_string());
        self.grpc_client.create_async(&req).unwrap()
    }

    /// Create `Endpoint` element.
    pub fn create_endpoint(
        &self,
        uri: &EndpointUri,
        endpoint: Endpoint,
    ) -> impl Future<Item = Response, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_id(uri.to_string());
        match endpoint {
            Endpoint::WebRtcPlayEndpoint { spec } => {
                req.set_webrtc_play(spec.into());
            }
            Endpoint::WebRtcPublishEndpoint { spec } => {
                req.set_webrtc_pub(spec.into());
            }
        }

        self.grpc_client.create_async(&req).unwrap()
    }

    /// Single get element.
    pub fn get_single<T: fmt::Display>(
        &self,
        uri: T,
    ) -> impl Future<Item = GetResponse, Error = Error> {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client.get_async(&req).unwrap()
    }

    /// Get batch of elements.
    pub fn get_batch(
        &self,
        uris: Vec<String>,
    ) -> impl Future<Item = GetResponse, Error = Error> {
        let req = id_request(uris);

        self.grpc_client.get_async(&req).unwrap()
    }

    /// Delete single element.
    pub fn delete_single<T: fmt::Display>(
        &self,
        uri: T,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client.delete_async(&req).unwrap()
    }

    /// Delete batch of elements.
    pub fn delete_batch(
        &self,
        ids: Vec<String>,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(ids);

        self.grpc_client.delete_async(&req).unwrap()
    }
}

/// Get gRPC client for control API.
fn get_grpc_client(addr: &str) -> ControlApiClient {
    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect(addr);
    ControlApiClient::new(ch)
}
