//! Repository which stores and lazily creates callback services.

use std::{
    collections::HashMap,
    fmt::{Debug, Error, Formatter},
    sync::{Arc, Mutex},
};

use actix::{Actor as _, Recipient};

use crate::api::control::callback::callback_url::CallbackUrl;

use super::{grpc_callback_service::GrpcCallbackService, Callback};

struct Inner(HashMap<CallbackUrl, Recipient<Callback>>);

impl Inner {
    /// Creates and inserts callback service based on provided [`CallbackUrl`].
    ///
    /// Note that this function will overwrite service if it already presented
    /// in storage.
    fn create_service(&mut self, url: &CallbackUrl) -> Recipient<Callback> {
        let callback_service = match url {
            CallbackUrl::Grpc(grpc_url) => {
                let grpc_service = GrpcCallbackService::new(grpc_url).start();
                grpc_service.recipient()
            }
        };
        self.0.insert(url.clone(), callback_service.clone());

        callback_service
    }

    fn get(&mut self, url: &CallbackUrl) -> Recipient<Callback> {
        if let Some(callback_service) = self.0.get(url).clone() {
            callback_service.clone()
        } else {
            self.create_service(url)
        }
    }
}

impl Debug for Inner {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
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
    pub fn get(&self, url: &CallbackUrl) -> Recipient<Callback> {
        self.0.lock().unwrap().get(url)
    }
}
