//! Service which stores and lazily creates [`CallbackRequest`] clients.

use std::{collections::hash_map::HashMap, fmt::Debug, sync::Arc};

use actix::Arbiter;
use parking_lot::RwLock;

use crate::{
    api::control::{
        callback::{url::CallbackUrl, CallbackEvent},
        refs::StatefulFid,
    },
    log::prelude::*,
};

use super::{
    clients::{build_client, CallbackClient},
    CallbackRequest,
};

/// Service which stores and lazily creates [`CallbackRequest`] clients.
#[derive(Clone, Debug, Default)]
pub struct CallbackService(
    // TODO: Hashmap entries are not dropped anywhere. some kind of
    //       [expiring map](https://github.com/jhalterman/expiringmap)
    //       would fit here.
    Arc<RwLock<HashMap<CallbackUrl, Box<dyn CallbackClient>>>>,
);

impl CallbackService {
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
        let inner = self.0.clone();
        Arbiter::spawn(async move {
            let req = CallbackRequest::new(fid, event.into());
            info!("Sending CallbackRequest [{:?}] to [{}]", req, callback_url);

            let mut read_lock = inner.write();
            let send_request =
                if let Some(client) = read_lock.get_mut(&callback_url) {
                    client.send(req).await
                } else {
                    drop(read_lock);
                    let mut new_client = build_client(&callback_url).await;
                    let send = new_client.send(req).await;
                    inner.write().insert(callback_url, Box::new(new_client));
                    send
                };

            if let Err(e) = send_request {
                error!("Failed to send callback because {:?}.", e);
            }
        })
    }
}
