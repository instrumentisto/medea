//! Implementations of Control API callback clients for all protocols.

pub mod grpc;

use std::fmt::Debug;

use futures::Future;
use grpcio::Error;

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
    ) -> Box<dyn Future<Item = (), Error = CallbackClientError>>;
}

#[derive(Debug)]
pub enum CallbackClientError {
    Grpcio(grpcio::Error),
}

impl From<grpcio::Error> for CallbackClientError {
    fn from(err: Error) -> Self {
        Self::Grpcio(err)
    }
}

/// Creates [`CallbackClient`] based on provided [`CallbackUrl`].
pub fn build_client(url: &CallbackUrl) -> impl CallbackClient {
    info!("Creating CallbackClient for url: {}", url);
    match &url {
        CallbackUrl::Grpc(grpc_url) => grpc::GrpcCallbackClient::new(grpc_url),
    }
}
