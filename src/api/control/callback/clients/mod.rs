//! Implementations of Control API callback clients for all protocols.

pub mod grpc;

use std::fmt::Debug;

use async_trait::async_trait;
use derive_more::From;

use crate::{
    api::control::callback::{url::CallbackUrl, CallbackRequest},
    log::prelude::*,
};

/// Error of sending [`CallbackRequest`] by [`CallbackClient`].
#[derive(Debug, From)]
pub enum CallbackClientError {
    /// [`tonic`] failed to send [`CallbackRequest`].
    Tonic(tonic::Status),

    /// Error while creating new [`CallbackClient`].
    TonicTransport(tonic::transport::Error),
}

#[async_trait]
pub trait CallbackClient: Debug + Send + Sync {
    async fn send(
        &self,
        request: CallbackRequest,
    ) -> Result<(), CallbackClientError>;
}

/// Creates [`CallbackClient`] basing on provided [`CallbackUrl`].
#[inline]
pub async fn build_client(
    url: &CallbackUrl,
) -> Result<impl CallbackClient, CallbackClientError> {
    info!("Creating CallbackClient for URL: {}", url);
    match &url {
        CallbackUrl::Grpc(grpc_url) => {
            grpc::GrpcCallbackClient::new(grpc_url).await
        }
    }
}
