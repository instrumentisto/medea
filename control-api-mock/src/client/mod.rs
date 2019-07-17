use grpcio::{ChannelBuilder, EnvBuilder, Error};
use protobuf::RepeatedField;
use std::sync::Arc;

use futures::Future;
use medea::api::control::grpc::protos::{
    control::{IdRequest, Response},
    control_grpc::ControlApiClient,
};

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
        room_id: String,
    ) -> impl Future<Item = Response, Error = Error> {
        let mut delete_req = IdRequest::new();
        let mut ids = RepeatedField::new();
        let uri = format!("local://{}", room_id);
        ids.push(uri);
        delete_req.set_id(ids);

        self.grpc_client.delete_async(&delete_req).unwrap()
    }
}

fn get_grpc_client() -> ControlApiClient {
    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect("localhost:50051");
    ControlApiClient::new(ch)
}
