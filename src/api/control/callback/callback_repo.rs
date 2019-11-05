use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use actix::{Actor as _, Addr};
use actix::Recipient;

use super::{
    callback_url::GrpcCallbackUrl, grpc_callback_service::GrpcCallbackService,
    Callback,
};
use std::fmt::Debug;
use failure::_core::fmt::{Formatter, Error};
use crate::api::control::callback::callback_url::CallbackUrl;

//#[derive(Debug)]
struct Inner {
    grpc: HashMap<CallbackUrl, Recipient<Callback>>,
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
        Self(Arc::new(Mutex::new(Inner {
            grpc: HashMap::new(),
        })))
    }

    pub fn get(
        &self,
        addr: &CallbackUrl,
    ) -> Recipient<Callback> {
        let mut locked_self = self.0.lock().unwrap();
        if let Some(grpc_service) = locked_self.grpc.get(addr) {
            grpc_service.clone()
        } else {
            let callback_service = create_callback_service(addr);
            locked_self.grpc.insert(addr.clone(), callback_service.clone());
            callback_service
        }
    }
}

fn create_callback_service(url: &CallbackUrl) -> Recipient<Callback> {
    match url {
        CallbackUrl::Grpc(grpc_url) => {
            let grpc_service = GrpcCallbackService::new(grpc_url).start();
            grpc_service.recipient()
        }
    }
}
