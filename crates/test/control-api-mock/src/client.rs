//! Implementation of client for Medea's gRPC Control API.

use std::{fmt, sync::Arc};

use actix_web::web::Path;
use futures::{Future, IntoFuture};
use grpcio::{ChannelBuilder, EnvBuilder, Error};
use medea_control_api_proto::grpc::{
    control_api::{
        CreateRequest, CreateResponse, GetResponse, IdRequest, Response,
    },
    control_api_grpc::ControlApiClient,
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

/// URI to `Member` element.
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

/// URI to `Endpoint` element.
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

/// Returns new [`IdRequest`] with provided IDs.
fn id_request(ids: Vec<String>) -> IdRequest {
    let mut req = IdRequest::new();
    let ids = RepeatedField::from(ids);
    req.set_id(ids);
    req
}

/// Client for Medea's Control API.
#[allow(clippy::module_name_repetitions)]
pub struct ControlClient {
    /// [`grpcio`] gRPC client for Medea Control API.
    grpc_client: ControlApiClient,
}

impl ControlClient {
    /// Creates new client for Medea's control API.
    ///
    /// __Note that call of this function don't checks availability of Control
    /// API gRPC server. Availability checks only on sending request to gRPC
    /// server.__
    pub fn new(medea_addr: &str) -> Self {
        Self {
            grpc_client: new_grpcio_control_api_client(medea_addr),
        }
    }

    /// Creates `Room` element with provided [`RoomUri`] and `Room` spec.
    pub fn create_room(
        &self,
        uri: &RoomUri,
        room: Room,
    ) -> impl Future<Item = CreateResponse, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_room(room.into());
        req.set_id(uri.to_string());

        self.grpc_client
            .create_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Creates `Member` element with provided [`MemberUri`] and `Member` spec.
    pub fn create_member(
        &self,
        uri: &MemberUri,
        member: Member,
    ) -> impl Future<Item = CreateResponse, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_member(member.into());
        req.set_id(uri.to_string());

        self.grpc_client
            .create_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Creates `Endpoint` element with provided [`EndpointUri`] and `Endpoint`
    /// spec.
    pub fn create_endpoint(
        &self,
        uri: &EndpointUri,
        endpoint: Endpoint,
    ) -> impl Future<Item = CreateResponse, Error = Error> {
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

        self.grpc_client
            .create_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Gets single element from Control API by local URI.
    pub fn get_single<T>(
        &self,
        uri: T,
    ) -> impl Future<Item = GetResponse, Error = Error>
    where
        T: fmt::Display,
    {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client
            .get_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Gets all elements with provided Local URIs.
    pub fn get_batch(
        &self,
        uris: Vec<String>,
    ) -> impl Future<Item = GetResponse, Error = Error> {
        let req = id_request(uris);

        self.grpc_client
            .get_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Deletes single element.
    pub fn delete_single<T>(
        &self,
        uri: T,
    ) -> impl Future<Item = Response, Error = Error>
    where
        T: fmt::Display,
    {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client
            .delete_async(&req)
            .into_future()
            .and_then(|r| r)
    }

    /// Deletes all elements with provided local URIs.
    pub fn delete_batch(
        &self,
        ids: Vec<String>,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(ids);

        self.grpc_client
            .delete_async(&req)
            .into_future()
            .and_then(|r| r)
    }
}

/// Returns new [`grpcio`] gRPC client for Control API.
fn new_grpcio_control_api_client(addr: &str) -> ControlApiClient {
    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect(addr);
    ControlApiClient::new(ch)
}
