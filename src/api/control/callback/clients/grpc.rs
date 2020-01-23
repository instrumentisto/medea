//! Implementation of gRPC client for sending [`CallbackRequest`]s.

use std::{fmt, sync::Arc};

use futures::{
    compat::Future01CompatExt as _,
    future::{FutureExt as _, LocalBoxFuture},
};
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
pub struct GrpcCallbackClient {
    /// [`grpcio`] gRPC client of Control API Callback service.
    client: ProtoCallbackClient,
}

impl fmt::Debug for GrpcCallbackClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("GrpcCallbackClient")
            .field("client", &"/* Cannot be printed */")
            .finish()
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
    ) -> LocalBoxFuture<'static, Result<(), CallbackClientError>> {
        let request = self.client.on_event_async(&request.into());
        async {
            request?.compat().await?;
            Ok(())
        }
        .boxed_local()
    }
}
