use std::{
    collections::HashMap,
    fmt::{Debug, Error, Formatter},
    sync::{Arc, Mutex},
};

use actix::{Actor as _, Addr, Recipient};

use crate::api::control::callback::callback_url::CallbackUrl;

use super::{
    callback_url::GrpcCallbackUrl, grpc_callback_service::GrpcCallbackService,
    Callback,
};

struct Inner(HashMap<CallbackUrl, Recipient<Callback>>);

impl Inner {
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
        write!(f, "BLAH-BLAH-BLAH")
    }
}

#[derive(Debug, Clone)]
pub struct CallbackRepository(Arc<Mutex<Inner>>);

impl CallbackRepository {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Inner(HashMap::new()))))
    }

    pub fn get(&self, url: &CallbackUrl) -> Recipient<Callback> {
        self.0.lock().unwrap().get(url)
    }
}
