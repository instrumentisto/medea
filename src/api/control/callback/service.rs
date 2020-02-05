//! Service which stores and lazily creates [`CallbackRequest`] clients.

use std::{
    collections::hash_map::HashMap, fmt::Debug, marker::PhantomData, sync::Arc,
};

use actix::Arbiter;
use tokio::sync::RwLock;

use crate::{
    api::control::{
        callback::{
            clients::{
                CallbackClient, CallbackClientError, CallbackClientFactory,
            },
            url::CallbackUrl,
            CallbackEvent, CallbackRequest,
        },
        refs::StatefulFid,
    },
    log::prelude::*,
};

type CallbackClientArc = Arc<Box<dyn CallbackClient>>;

/// Service which stores and lazily creates [`CallbackRequest`] clients.
#[derive(Debug, Default)]
pub struct CallbackService<B>(
    // TODO: Hashmap entries are not dropped anywhere. some kind of
    //       [expiring map](https://github.com/jhalterman/expiringmap)
    //       would fit here.
    Arc<RwLock<HashMap<CallbackUrl, CallbackClientArc>>>,
    PhantomData<B>,
);

impl<B> Clone for CallbackService<B> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
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

        let read_lock = self.0.read().await;
        let client = if let Some(client) = read_lock.get(&callback_url) {
            Arc::clone(client)
        } else {
            drop(read_lock);

            // We are building client while holding write lock to avoid races,
            // that can lead to creating multiple clients to same uri.
            let mut write_lock = self.0.write().await;
            if let Some(client) = write_lock.get(&callback_url) {
                Arc::clone(client)
            } else {
                let new_client: CallbackClientArc =
                    Arc::new(B::build(callback_url.clone()).await?);
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
    pub fn send_callback<T: Into<CallbackEvent> + 'static>(
        &self,
        callback_url: CallbackUrl,
        fid: StatefulFid,
        event: T,
    ) {
        let this = self.clone();
        Arbiter::spawn(async move {
            let req = CallbackRequest::new(fid, event.into());

            if let Err(e) = this.send_request(req, callback_url).await {
                error!("Failed to send callback because {:?}.", e);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom as _, time::Duration};

    use futures::channel::oneshot;
    use serial_test_derive::serial;

    use crate::api::control::callback::{
        clients::{MockCallbackClient, MockCallbackClientFactory},
        OnJoinEvent,
    };

    use super::*;

    /// Returns [`CallbackRequest`] to a `foo` element.
    fn callback_request() -> CallbackRequest {
        CallbackRequest::new(
            StatefulFid::try_from("foo".to_string()).unwrap(),
            CallbackEvent::OnJoin(OnJoinEvent),
        )
    }

    /// Returns [`CallbackUrl`] to a `grpc://127.0.0.1:6565`.
    fn callback_url() -> CallbackUrl {
        CallbackUrl::try_from("grpc://127.0.0.1:6565".to_string()).unwrap()
    }

    /// Tests that only 1 [`CallbackClient`] will be created on 10 calls of
    /// [`CallbackService::send_request`].
    #[actix_rt::test]
    #[serial]
    async fn only_one_client_will_be_created() {
        const SEND_COUNT: usize = 10;

        let mut client_mock = MockCallbackClient::new();
        client_mock
            .expect_send()
            .times(SEND_COUNT)
            .returning(|_| Box::pin(async { Ok(()) }));

        let client_builder_ctx = MockCallbackClientFactory::build_context();
        client_builder_ctx.expect().times(1).return_once(move |_| {
            Box::pin(async move {
                Ok(Box::new(client_mock) as Box<dyn CallbackClient>)
            })
        });

        let callback_service =
            CallbackService::<MockCallbackClientFactory>::default();
        for _ in 0..SEND_COUNT {
            callback_service
                .send_request(callback_request(), callback_url())
                .await
                .unwrap();
        }
    }

    /// Tests that two simultaneous calls of [`CallbackService::send_request`]
    /// from two threads will create only one [`CallbackClient`].
    #[actix_rt::test]
    #[serial]
    async fn only_one_client_will_be_created_on_multithread() {
        let mut client_mock = MockCallbackClient::new();
        client_mock
            .expect_send()
            .times(2)
            .returning(|_| Box::pin(async { Ok(()) }));

        let client_builder_ctx = MockCallbackClientFactory::build_context();
        client_builder_ctx.expect().times(1).return_once(move |_| {
            Box::pin(async move {
                tokio::time::delay_for(Duration::from_millis(50)).await;
                Ok(Box::new(client_mock) as Box<dyn CallbackClient>)
            })
        });

        let callback_service =
            CallbackService::<MockCallbackClientFactory>::default();

        let (wait_for_another_arbiter_tx, wait_for_another_arbiter_rx) =
            oneshot::channel();
        let another_arbiter_callback_service = callback_service.clone();
        Arbiter::new().exec_fn(move || {
            futures::executor::block_on(Box::pin(async move {
                another_arbiter_callback_service
                    .send_request(callback_request(), callback_url())
                    .await
                    .unwrap();
                wait_for_another_arbiter_tx.send(()).unwrap();
            }))
        });

        callback_service
            .send_request(callback_request(), callback_url())
            .await
            .unwrap();

        wait_for_another_arbiter_rx.await.unwrap();
    }
}
