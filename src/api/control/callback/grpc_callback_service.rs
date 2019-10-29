use std::{
    fmt,
    fmt::{Error, Formatter},
    sync::Arc,
};

use actix::{Actor, Context};
use grpcio::{ChannelBuilder, EnvBuilder};
use medea_control_api_proto::grpc::callback_grpc::CallbackClient as GrpcioCallbackClient;

use super::callback_url::GrpcCallbackUrl;

pub struct GrpcCallbackService {
    client: GrpcioCallbackClient,
}

impl fmt::Debug for GrpcCallbackService {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(
            f,
            "GrpcCallbackService {{ client: /* Cannot be printed */ }}"
        )
    }
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
