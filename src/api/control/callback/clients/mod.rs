//! Implementations of Control API callback clients for all protocols.

pub mod grpc;

use std::fmt::Debug;

use derive_more::From;
use futures::Future;

use crate::{
    api::control::callback::{url::CallbackUrl, CallbackRequest},
    log::prelude::*,
};

/// Client that sends [`CallbackRequest`]'s to [`Callback`] server.
pub trait CallbackClient: Debug + Send + Sync {
    /// Sends [`CallbackRequest`] to [`Callback`] server.
    fn send(
        &self,
        request: CallbackRequest,
    ) -> Box<dyn Future<Output = Result<(), CallbackClientError>>>;
}

/// Error of sending [`CallbackRequest`] by [`CallbackClient`].
#[derive(Debug, From)]
pub enum CallbackClientError {
    /// [`grpcio`] failed to send [`CallbackRequest`].
    Grpcio(grpcio::Error),
}

/// Creates [`CallbackClient`] basing on provided [`CallbackUrl`].
#[inline]
pub fn build_client(url: &CallbackUrl) -> impl CallbackClient {
    info!("Creating CallbackClient for url: {}", url);
    match &url {
        CallbackUrl::Grpc(grpc_url) => grpc::GrpcCallbackClient::new(grpc_url),
    }
}
