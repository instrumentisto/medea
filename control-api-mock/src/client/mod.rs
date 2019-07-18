use std::sync::Arc;

use futures::Future;
use grpcio::{ChannelBuilder, EnvBuilder, Error};
use medea::api::control::grpc::protos::{
    control::{IdRequest, Response},
    control_grpc::ControlApiClient,
};
use protobuf::RepeatedField;

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
        room_id: &str,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![room_local_uri(room_id)]);

        self.grpc_client.delete_async(&req).unwrap()
    }

    pub fn delete_member(
        &self,
        room_id: &str,
        member_id: &str,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![member_local_uri(room_id, member_id)]);

        self.grpc_client.delete_async(&req).unwrap()
    }

    pub fn delete_endpoint(
        &self,
        room_id: &str,
        member_id: &str,
        endpoint_id: &str,
    ) -> impl Future<Item = Response, Error = Error> {
        let req = id_request(vec![endpoint_local_uri(
            room_id,
            member_id,
            endpoint_id,
        )]);

        self.grpc_client.delete_async(&req).unwrap()
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
