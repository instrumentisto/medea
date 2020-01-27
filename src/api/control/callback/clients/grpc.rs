//! Implementation of gRPC client for sending [`CallbackRequest`]s.

use std::{fmt, sync::Arc};

use futures::future::{FutureExt as _, LocalBoxFuture};
#[rustfmt::skip]
use medea_control_api_proto::grpc::medea_callback::{
    callback_client::CallbackClient as ProtoCallbackClient
};

use crate::api::control::callback::{
    clients::{CallbackClient, CallbackClientError},
    url::GrpcCallbackUrl,
    CallbackRequest,
};
use std::sync::Mutex;
use tonic::transport::Channel;

/// gRPC client for sending [`CallbackRequest`]s.
pub struct GrpcCallbackClient {
    /// [`grpcio`] gRPC client of Control API Callback service.
    client: Arc<Mutex<ProtoCallbackClient<Channel>>>,
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
    pub async fn new(addr: &GrpcCallbackUrl) -> Self {
        let addr = addr.addr();
        let client = Arc::new(Mutex::new(
            ProtoCallbackClient::connect(addr).await.unwrap(),
        ));

        Self { client }
    }
}

impl CallbackClient for GrpcCallbackClient {
    fn send(
        &self,
        request: CallbackRequest,
    ) -> LocalBoxFuture<'static, Result<(), CallbackClientError>> {
        let client = Arc::clone(&self.client);
        async move {
            client
                .lock()
                .unwrap()
                .on_event(tonic::Request::new(request.into()))
                .await?;
            Ok(())
        }
        .boxed_local()
    }
}
