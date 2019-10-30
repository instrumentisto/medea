use std::{
    fmt,
    fmt::{Error, Formatter},
    sync::Arc,
};

use actix::{Actor, Context, Handler, ResponseFuture};
use futures::future::{Future as _, IntoFuture as _};
use grpcio::{ChannelBuilder, EnvBuilder};
use medea_control_api_proto::grpc::{
    callback::Request, callback_grpc::CallbackClient as GrpcioCallbackClient,
};

use crate::api::control::callback::Callback;

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

impl Handler<Callback> for GrpcCallbackService {
    type Result = ResponseFuture<(), ()>;

    fn handle(
        &mut self,
        msg: Callback,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut req = Request::new();
        req.set_event(msg.event.into());
        req.set_element(msg.element.to_string());
        req.set_at(msg.at.to_rfc3339());

        Box::new(
            self.client
                .on_event_async(&req)
                .into_future()
                .and_then(|q| q)
                .map(|_| ())
                .map_err(|_| ()),
        )
    }
}
