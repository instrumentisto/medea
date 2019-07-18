use std::{fmt, sync::Arc};

use actix_web::web::Path;
use futures::Future;
use grpcio::{ChannelBuilder, EnvBuilder, Error};
use medea::api::control::grpc::protos::{
    control::{CreateRequest, IdRequest, Response},
    control_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

use crate::server::{
    endpoint::{Endpoint, EndpointPath},
    member::{Member, MemberPath},
    room::{Room, RoomPath},
};
use medea::api::control::grpc::protos::control::GetResponse;

#[derive(Clone, Debug)]
pub struct RoomUri {
    room_id: String,
}

impl From<Path<RoomPath>> for RoomUri {
    fn from(path: Path<RoomPath>) -> Self {
        Self {
            room_id: path.room_id.clone(),
        }
    }
}

impl fmt::Display for RoomUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local://{}", self.room_id)
    }
}

#[derive(Clone, Debug)]
pub struct MemberUri {
    room_id: String,
    member_id: String,
}

impl From<Path<MemberPath>> for MemberUri {
    fn from(path: Path<MemberPath>) -> Self {
        Self {
            room_id: path.room_id.clone(),
            member_id: path.member_id.clone(),
        }
    }
}

impl fmt::Display for MemberUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local://{}/{}", self.room_id, self.member_id)
    }
}

#[derive(Clone, Debug)]
pub struct EndpointUri {
    room_id: String,
    member_id: String,
    endpoint_id: String,
}

impl From<Path<EndpointPath>> for EndpointUri {
    fn from(path: Path<EndpointPath>) -> Self {
        Self {
            room_id: path.room_id.clone(),
            member_id: path.member_id.clone(),
            endpoint_id: path.endpoint_id.clone(),
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

fn id_request(ids: Vec<String>) -> IdRequest {
    let mut req = IdRequest::new();
    let ids = RepeatedField::from(ids);
    req.set_id(ids);
    req
}

#[allow(clippy::module_name_repetitions)]
pub struct ControlClient {
    grpc_client: ControlApiClient,
}

impl ControlClient {
    pub fn new() -> Self {
        Self {
            grpc_client: get_grpc_client(),
        }
    }

    pub fn delete_room(
        &self,
        uri: RoomUri,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client.delete_async(&req).unwrap()
    }

    pub fn create_room(
        &self,
        uri: RoomUri,
        room: Room,
    ) -> impl Future<Item = Response, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_room(room.into());
        req.set_id(uri.to_string());

        self.grpc_client.create_async(&req).unwrap()
    }

    pub fn delete_member(
        &self,
        uri: MemberUri,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client.delete_async(&req).unwrap()
    }

    pub fn create_member(
        &self,
        uri: MemberUri,
        member: Member,
    ) -> impl Future<Item = Response, Error = Error> {
        let mut req = CreateRequest::new();
        req.set_member(member.into());
        req.set_id(uri.to_string());
        self.grpc_client.create_async(&req).unwrap()
    }

    pub fn delete_endpoint(
        &self,
        uri: EndpointUri,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client.delete_async(&req).unwrap()
    }

    pub fn create_endpoint(
        &self,
        uri: EndpointUri,
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

    pub fn get_single<T: fmt::Display>(
        &self,
        uri: T,
    ) -> impl Future<Item = GetResponse, Error = Error> {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client.get_async(&req).unwrap()
    }

    pub fn get_batch(
        &self,
        uris: Vec<String>,
    ) -> impl Future<Item = GetResponse, Error = Error> {
        let req = id_request(uris);

        self.grpc_client.get_async(&req).unwrap()
    }
}

impl Default for ControlClient {
    fn default() -> Self {
        Self::new()
    }
}

fn get_grpc_client() -> ControlApiClient {
    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect("localhost:50051");
    ControlApiClient::new(ch)
}
