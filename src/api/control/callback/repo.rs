//! Repository which stores and lazily creates callback services.

use std::{
    collections::HashMap,
    fmt::{self, Debug},
    sync::{Arc, Mutex},
};

use actix::{Actor as _, Arbiter, Recipient};
use futures::future::Future as _;

use crate::{
    api::control::{
        callback::{url::CallbackUrl, CallbackEvent},
        refs::StatefulFid,
    },
    log::prelude::*,
};

use super::{services::grpc::GrpcCallbackService, Callback};

struct Inner(HashMap<CallbackUrl, Recipient<Callback>>);

impl Inner {
    /// Creates and inserts callback service based on provided [`CallbackUrl`].
    ///
    /// Note that this function will overwrite service if it already presented
    /// in storage.
    fn create_service(&mut self, url: CallbackUrl) -> Recipient<Callback> {
        let callback_service = match &url {
            CallbackUrl::Grpc(grpc_url) => {
                GrpcCallbackService::new(grpc_url).start().recipient()
            }
        };
        self.0.insert(url, callback_service.clone());

        callback_service
    }

    fn get(&mut self, url: CallbackUrl) -> Recipient<Callback> {
        if let Some(callback_service) = self.0.get(&url) {
            callback_service.clone()
        } else {
            self.create_service(url)
        }
    }
}

impl Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Inner {{ /* Cannot be printed */ }}")
    }
}

#[derive(Debug, Clone)]
pub struct CallbackRepository(Arc<Mutex<Inner>>);

impl CallbackRepository {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Inner(HashMap::new()))))
    }

    /// Returns or creates (if not presented in storage) callback service.
    ///
    /// You can provide any [`CallbackUrl`] to this function regardless of
    /// protocol. Also you can don't worry about what protocol is used
    /// because this function returns [`Recipient`].
    ///
    /// If some service not presented in repository then new service
    /// automatically will be created.
    fn get(&self, url: CallbackUrl) -> Recipient<Callback> {
        self.0.lock().unwrap().get(url)
    }

    /// Sends [`CallbackEvent`] for provided [`StatefulFid`] to
    /// callback service.
    // TODO: Add buffering and resending for failed 'Callback' sends.
    //       https://github.com/instrumentisto/medea/issues/61
    pub fn send_callback<T: Into<CallbackEvent>>(
        &self,
        callback_url: CallbackUrl,
        fid: StatefulFid,
        event: T,
    ) {
        Arbiter::spawn(
            self.get(callback_url)
                .send(Callback::new(fid, event.into()))
                .map_err(|e| error!("Failed to send callback because {:?}.", e))
                .map(|res| {
                    if let Err(err) = res {
                        warn!("Failed to send callback because {:?}.", err);
                    }
                }),
        );
    }
}

impl Default for CallbackRepository {
    fn default() -> Self {
        Self::new()
    }
}
