use std::sync::Arc;

use actix::{Actor, Context};
use grpcio::{ChannelBuilder, EnvBuilder};
use medea_control_api_proto::grpc::callback_grpc::CallbackClient as GrpcioCallbackClient;

use crate::api::control::callback_url::GrpcCallbackUrl;

pub struct GrpcCallbackService {
    client: GrpcioCallbackClient,
}

impl GrpcCallbackService {
    pub fn new(addr: &GrpcCallbackUrl) -> Self {
        let env = Arc::new(EnvBuilder::new().build());
        let ch = ChannelBuilder::new(env).connect(&addr.to_string());
        let client = GrpcioCallbackClient::new(ch);

        Self { client }
    }
}

impl Actor for GrpcCallbackService {
    type Context = Context<Self>;
}
