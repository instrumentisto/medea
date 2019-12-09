//! Implementation of gRPC client for sending [`CallbackRequest`]s.

use std::{
    fmt,
    fmt::{Error, Formatter},
    sync::Arc,
};

use futures::future::{Future, IntoFuture as _};
use grpcio::{ChannelBuilder, EnvBuilder};
#[rustfmt::skip]
use medea_control_api_proto::grpc::callback_grpc::{
    CallbackClient as ProtoCallbackClient
};

use crate::api::control::callback::{
    clients::{CallbackClient, CallbackClientError},
    url::GrpcCallbackUrl,
    CallbackRequest,
};

/// gRPC client for sending [`CallbackRequest`]s.
#[allow(clippy::module_name_repetitions)]
pub struct GrpcCallbackClient {
    /// [`grpcio`] gRPC client for Control API callback.
    client: ProtoCallbackClient,
}

impl fmt::Debug for GrpcCallbackClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(
            f,
            "GrpcCallbackClient {{ client: /* Cannot be printed */ }}"
        )
    }
}

impl GrpcCallbackClient {
    /// Returns gRPC client for provided [`GrpcCallbackUrl`].
    ///
    /// Note that this function doesn't check availability of gRPC server on
    /// provided [`GrpcCallbackUrl`].
    pub fn new(addr: &GrpcCallbackUrl) -> Self {
        let env = Arc::new(EnvBuilder::new().build());
        let ch = ChannelBuilder::new(env).connect(addr.addr());
        let client = ProtoCallbackClient::new(ch);

        Self { client }
    }
}

impl CallbackClient for GrpcCallbackClient {
    fn send(
        &self,
        request: CallbackRequest,
    ) -> Box<dyn Future<Item = (), Error = CallbackClientError>> {
        Box::new(
            self.client
                .on_event_async(&request.into())
                .into_future()
                .and_then(|f| f)
                .map(|_| ())
                .map_err(CallbackClientError::from),
        )
    }
}
