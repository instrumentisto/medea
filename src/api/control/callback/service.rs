//! Service which stores and lazily creates [`CallbackRequest`] clients.

use std::{
    collections::hash_map::HashMap, fmt::Debug, marker::PhantomData, sync::Arc,
};

use actix::Arbiter;
use tokio::sync::RwLock;

use crate::{
    api::control::callback::{
        clients::{CallbackClient, CallbackClientError, CallbackClientFactory},
        url::CallbackUrl,
        CallbackRequest,
    },
    log::prelude::*,
};

/// Service which stores and lazily creates [`CallbackRequest`] clients.
#[derive(Debug, Default)]
pub struct CallbackService<B> {
    // TODO: Hashmap entries are not dropped anywhere. some kind of
    //       [expiring map](https://github.com/jhalterman/expiringmap)
    //       would fit here.
    clients: Arc<RwLock<HashMap<CallbackUrl, Arc<dyn CallbackClient>>>>,
    _factory: PhantomData<B>,
}

impl<B> Clone for CallbackService<B> {
    fn clone(&self) -> Self {
        Self {
            clients: self.clients.clone(),
            _factory: PhantomData,
        }
    }
}

impl<B: CallbackClientFactory + 'static> CallbackService<B> {
    async fn send_request(
        &self,
        request: CallbackRequest,
        callback_url: CallbackUrl,
    ) -> Result<(), CallbackClientError> {
        info!(
            "Sending CallbackRequest [{:?}] to [{}]",
            request, callback_url
        );

        let read_lock = self.clients.read().await;
        let client = if let Some(client) = read_lock.get(&callback_url) {
            Arc::clone(client)
        } else {
            drop(read_lock);
            let mut write_lock = self.clients.write().await;
            // Double checked locking is kinda redundant atm, since this future
            // is `!Send`, but lets leave it this way for additional
            // future-proofing.
            if let Some(client) = write_lock.get(&callback_url) {
                Arc::clone(client)
            } else {
                // We are building client while holding write lock to
                // avoid races, that can lead to creating
                // multiple clients to same uri.
                let new_client = B::build(callback_url.clone()).await?;
                write_lock.insert(callback_url, Arc::clone(&new_client));

                new_client
            }
        };

        client.send(request).await?;

        Ok(())
    }

    /// Asynchronously sends [`CallbackEvent`] for provided [`StatefulFid`] to
    /// [`CallbackClient`].
    ///
    /// Will use existing [`CallbackClient`] or create new.
    // TODO: Add buffering and resending for failed 'Callback' sends.
    //       https://github.com/instrumentisto/medea/issues/61
    pub fn send_callback(
        &self,
        callback_url: CallbackUrl,
        request: CallbackRequest,
    ) {
        let this = self.clone();
        Arbiter::spawn(async move {
            if let Err(e) = this.send_request(request, callback_url).await {
                error!("Failed to send callback because {:?}.", e);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom as _, time::Duration};

    use futures::{future, FutureExt};
    use serial_test_derive::serial;
    use tokio::time;

    use crate::api::control::{
        callback::{
            clients::{MockCallbackClient, MockCallbackClientFactory},
            CallbackEvent, OnJoinEvent,
        },
        refs::StatefulFid,
    };

    use super::*;

    /// Returns [`CallbackRequest`] to a `foo` element.
    fn callback_request() -> CallbackRequest {
        CallbackRequest::new_at_now(
            StatefulFid::try_from("foo".to_string()).unwrap(),
            CallbackEvent::Join(OnJoinEvent),
        )
    }

    /// Returns [`CallbackUrl`] to a `grpc://127.0.0.1:6565`.
    fn callback_url() -> CallbackUrl {
        CallbackUrl::try_from("grpc://127.0.0.1:6565".to_string()).unwrap()
    }

    /// Tests that only 1 [`CallbackClient`] will be created if we perform
    /// multiple concurrent request.
    #[actix_rt::test]
    #[serial]
    async fn only_one_client_will_be_created() {
        const SEND_COUNT: usize = 20;

        let mut client_mock = MockCallbackClient::new();
        client_mock.expect_send().times(SEND_COUNT).returning(|_| {
            async {
                time::delay_for(Duration::from_millis(50)).await;
                Ok(())
            }
            .boxed_local()
        });

        let client_builder_ctx = MockCallbackClientFactory::build_context();
        client_builder_ctx.expect().times(1).return_once(move |_| {
            async move {
                time::delay_for(Duration::from_millis(50)).await;
                Ok(Arc::new(client_mock) as Arc<dyn CallbackClient>)
            }
            .boxed_local()
        });

        let callback_service =
            CallbackService::<MockCallbackClientFactory>::default();

        let tasks: Vec<_> = (0..SEND_COUNT)
            .map(|_| callback_service.clone())
            .map(|service| {
                async move {
                    service
                        .send_request(callback_request(), callback_url())
                        .await
                }
                .boxed_local()
            })
            .collect();
        future::join_all(tasks).await;
    }
}
