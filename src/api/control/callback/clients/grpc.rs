//! Implementation of gRPC client for sending [`CallbackRequest`]s.

use std::fmt;

#[rustfmt::skip]
use medea_control_api_proto::grpc::callback::{
    callback_client::CallbackClient as ProtoCallbackClient
};
use async_trait::async_trait;
use tonic::transport::Channel;

use crate::api::control::callback::{
    clients::{CallbackClient, CallbackClientError},
    url::GrpcCallbackUrl,
    CallbackRequest,
};

/// gRPC client for sending [`CallbackRequest`]s.
pub struct GrpcCallbackClient {
    /// [`tonic`] gRPC client of Control API Callback service.
    client: ProtoCallbackClient<Channel>,
}

impl GrpcCallbackClient {
    /// Returns gRPC client for provided [`GrpcCallbackUrl`].
    ///
    /// Note that this function doesn't check availability of gRPC server on
    /// provided [`GrpcCallbackUrl`].
    ///
    /// # Errors
    ///
    /// With [`CallbackClientError::TonicTransport`] if [`tonic`] transport
    /// cannot be created (so gRPC connection cannot be established).
    pub async fn new(
        addr: &GrpcCallbackUrl,
    ) -> Result<Self, CallbackClientError> {
        let addr = addr.addr();
        let client = ProtoCallbackClient::connect(addr).await?;
        Ok(Self { client })
    }
}

#[async_trait(?Send)]
impl CallbackClient for GrpcCallbackClient {
    async fn send(
        &self,
        request: CallbackRequest,
    ) -> Result<(), CallbackClientError> {
        let mut client = self.client.clone();
        client.on_event(tonic::Request::new(request.into())).await?;

        Ok(())
    }
}

impl fmt::Debug for GrpcCallbackClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("GrpcCallbackClient")
            .field("client", &"/* Cannot be printed */")
            .finish()
    }
}
