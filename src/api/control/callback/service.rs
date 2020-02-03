//! Service which stores and lazily creates [`CallbackRequest`] clients.

use std::{
    collections::hash_map::HashMap,
    fmt::Debug,
    sync::{Arc, RwLock},
};

use actix::Arbiter;

use crate::{
    api::control::{
        callback::{
            clients::{CallbackClient, CallbackClientError},
            url::CallbackUrl,
            CallbackEvent,
        },
        refs::StatefulFid,
    },
    log::prelude::*,
};

use super::{clients::build_client, CallbackRequest};

// TODO: wrap in actor

/// Service which stores and lazily creates [`CallbackRequest`] clients.
#[derive(Clone, Debug, Default)]
pub struct CallbackService(
    // TODO: Hashmap entries are not dropped anywhere. some kind of
    //       [expiring map](https://github.com/jhalterman/expiringmap)
    //       would fit here.
    Arc<RwLock<HashMap<CallbackUrl, Arc<dyn CallbackClient>>>>,
);

impl CallbackService {
    async fn send_request(
        &self,
        request: CallbackRequest,
        callback_url: CallbackUrl,
    ) -> Result<(), CallbackClientError> {
        info!(
            "Sending CallbackRequest [{:?}] to [{}]",
            request, callback_url
        );

        // TODO: refactor this when there will be some trustworthy thread safe
        //       HashMap with atomic `compute if absent`.
        let read_lock = self.0.read().unwrap();
        let client = if let Some(client) = read_lock.get(&callback_url) {
            Arc::clone(client)
        } else {
            drop(read_lock);

            // We are building client while holding write lock to avoid races,
            // that can lead to creating multiple clients to same uri.
            let mut write_lock = self.0.write().unwrap();
            if let Some(client) = write_lock.get(&callback_url) {
                Arc::clone(client)
            } else {
                let new_client: Arc<dyn CallbackClient> =
                    Arc::new(build_client(&callback_url).await?);
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
