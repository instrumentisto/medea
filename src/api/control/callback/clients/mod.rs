//! Implementations of Control API callback clients for all protocols.

pub mod grpc;

use std::{fmt, sync::Arc};

use async_trait::async_trait;
use derive_more::From;
use futures::future::{FutureExt, LocalBoxFuture};

use crate::{
    api::control::callback::{url::CallbackUrl, CallbackRequest},
    log::prelude::*,
};

type Result<T> = std::result::Result<T, CallbackClientError>;

/// Error of sending [`CallbackRequest`] by [`CallbackClient`].
#[derive(Debug, From)]
pub enum CallbackClientError {
    /// [`tonic`] failed to send [`CallbackRequest`].
    Tonic(tonic::Status),

    /// Error while creating new [`CallbackClient`].
    TonicTransport(tonic::transport::Error),
}

#[async_trait(?Send)]
#[cfg_attr(test, mockall::automock)]
pub trait CallbackClient: fmt::Debug + Send + Sync {
    /// Sends provided [`CallbackRequest`].
    async fn send(&self, request: CallbackRequest) -> Result<()>;
}

#[cfg(test)]
impl_debug_by_struct_name!(MockCallbackClient);

/// Factory for a [`CallbackClient`]s.
#[cfg_attr(test, mockall::automock)]
pub trait CallbackClientFactory {
    /// Creates [`CallbackClient`] basing on provided [`CallbackUrl`].
    fn build(
        url: CallbackUrl,
    ) -> LocalBoxFuture<'static, Result<Arc<dyn CallbackClient>>>;
}

#[cfg(test)]
impl_debug_by_struct_name!(MockCallbackClientFactory);

/// Implementation of the [`CallbackClientFactory`].
#[derive(Clone, Debug, Default)]
pub struct CallbackClientFactoryImpl;

impl CallbackClientFactory for CallbackClientFactoryImpl {
    #[inline]
    fn build(
        url: CallbackUrl,
    ) -> LocalBoxFuture<'static, Result<Arc<dyn CallbackClient>>> {
        info!("Creating CallbackClient for URL: {}", url);
        match url {
            CallbackUrl::Grpc(grpc_url) => async move {
                Ok(Arc::new(grpc::GrpcCallbackClient::new(&grpc_url).await?)
                    as Arc<dyn CallbackClient>)
            }
            .boxed_local(),
        }
    }
}
