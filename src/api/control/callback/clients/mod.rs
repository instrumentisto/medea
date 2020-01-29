//! Implementations of Control API callback clients for all protocols.

pub mod grpc;

use std::fmt::Debug;

use derive_more::From;
use futures::future::{BoxFuture, LocalBoxFuture};

use crate::{
    api::control::callback::{url::CallbackUrl, CallbackRequest},
    log::prelude::*,
};
use actix::{Actor, Addr, Recipient};
use std::sync::Arc;

/// Error of sending [`CallbackRequest`] by [`CallbackClient`].
#[derive(Debug, From)]
pub enum CallbackClientError {
    /// [`grpcio`] failed to send [`CallbackRequest`].
    Tonic(tonic::Status),

    Mailbox(actix::MailboxError),

    TonicTransport(tonic::transport::Error),
}

pub trait CallbackClient: Debug + Send + Sync {
    fn send(
        &self,
        request: CallbackRequest,
    ) -> LocalBoxFuture<'static, Result<(), CallbackClientError>>;
}

/// Creates [`CallbackClient`] basing on provided [`CallbackUrl`].
#[inline]
pub async fn build_client(
    url: &CallbackUrl,
) -> Result<impl CallbackClient, CallbackClientError> {
    info!("Creating CallbackClient for url: {}", url);
    match &url {
        CallbackUrl::Grpc(grpc_url) => {
            Ok(grpc::GrpcCallbackClient::new(grpc_url).await?.start())
        }
    }
}
