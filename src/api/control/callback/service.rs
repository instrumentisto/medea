//! Service which stores and lazily creates [`CallbackRequest`] clients.

use std::{collections::hash_map::HashMap, fmt::Debug, sync::Arc};

use actix::Arbiter;
use parking_lot::RwLock;

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

/// Service which stores and lazily creates [`CallbackRequest`] clients.
#[derive(Clone, Debug, Default)]
pub struct CallbackService(
    // TODO: Hashmap entries are not dropped anywhere. some kind of
    //       [expiring map](https://github.com/jhalterman/expiringmap)
    //       would fit here.
    Arc<RwLock<HashMap<CallbackUrl, Box<dyn CallbackClient>>>>,
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

        let read_lock = self.0.read();
        if let Some(client) = read_lock.get(&callback_url) {
            client.send(request).await?;
        } else {
            drop(read_lock);

            let new_client = build_client(&callback_url).await?;
            new_client.send(request).await?;
            self.0.write().insert(callback_url, Box::new(new_client));
        };

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
