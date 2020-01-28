//! Implementations of Control API callback clients for all protocols.

pub mod grpc;

use std::fmt::Debug;

use derive_more::From;
use futures::future::LocalBoxFuture;

use crate::{
    api::control::callback::{url::CallbackUrl, CallbackRequest},
    log::prelude::*,
};

/// Client that sends [`CallbackRequest`]'s to [`Callback`] server.
#[async_trait::async_trait]
pub trait CallbackClient: Debug + Send + Sync {
    /// Sends [`CallbackRequest`] to [`Callback`] server.
    async fn send(
        &mut self,
        request: CallbackRequest,
    ) -> Result<(), CallbackClientError>;
}

/// Error of sending [`CallbackRequest`] by [`CallbackClient`].
#[derive(Debug, From)]
pub enum CallbackClientError {
    /// [`grpcio`] failed to send [`CallbackRequest`].
    Tonic(tonic::Status),
}

/// Creates [`CallbackClient`] basing on provided [`CallbackUrl`].
#[inline]
pub async fn build_client(url: &CallbackUrl) -> impl CallbackClient {
    info!("Creating CallbackClient for url: {}", url);
    match &url {
        CallbackUrl::Grpc(grpc_url) => {
            grpc::GrpcCallbackClient::new(grpc_url).await
        }
    }
}
