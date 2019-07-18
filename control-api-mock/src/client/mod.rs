use std::{fmt, sync::Arc};

use crate::server::{
    endpoint::{Endpoint, EndpointPath},
    member::MemberPath,
    room::RoomPath,
};
use actix_web::web::Path;
use futures::Future;
use grpcio::{ChannelBuilder, EnvBuilder, Error};
use medea::api::control::grpc::protos::{
    control::{CreateRequest, IdRequest, Response, Room},
    control_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

#[derive(Clone, Debug)]
pub struct RoomUri {
    room_id: String,
}

impl From<Path<RoomPath>> for RoomUri {
    fn from(mut path: Path<RoomPath>) -> Self {
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

fn local_uri(text: &str) -> String {
    format!("local://{}", text)
}

fn room_local_uri(room_id: &str) -> String {
    local_uri(room_id)
}

fn member_local_uri(room_id: &str, member_id: &str) -> String {
    local_uri(&format!("{}/{}", room_id, member_id))
}

fn endpoint_local_uri(
    room_id: &str,
    member_id: &str,
    endpoint_id: &str,
) -> String {
    local_uri(&format!("{}/{}/{}", room_id, member_id, endpoint_id))
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

    pub fn delete_member(
        &self,
        uri: MemberUri,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![uri.to_string()]);

        self.grpc_client.delete_async(&req).unwrap()
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
