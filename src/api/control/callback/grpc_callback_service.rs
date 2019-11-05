use std::{
    fmt,
    fmt::{Error, Formatter},
    sync::Arc,
};

use actix::{Actor, Context, Handler, ResponseFuture};
use futures::future::{Future as _, IntoFuture as _};
use grpcio::{ChannelBuilder, EnvBuilder};
use medea_control_api_proto::grpc::callback_grpc::CallbackClient;

use crate::{api::control::callback::Callback, log::prelude::*};

use super::callback_url::GrpcCallbackUrl;

pub struct GrpcCallbackService {
    client: CallbackClient,
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
        let ch = ChannelBuilder::new(env).connect(addr.addr());
        let client = CallbackClient::new(ch);

        Self { client }
    }
}

impl Actor for GrpcCallbackService {
    type Context = Context<Self>;
}

impl Handler<Callback> for GrpcCallbackService {
    type Result = ResponseFuture<(), ()>;

    fn handle(&mut self, msg: Callback, _: &mut Self::Context) -> Self::Result {
        Box::new(
            self.client
                .on_event_async(&msg.into())
                .into_future()
                .and_then(|q| q)
                .map(|_| ())
                .map_err(|e| warn!("{:?}", e)),
        )
    }
}
