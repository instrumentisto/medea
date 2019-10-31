use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use actix::{Actor as _, Addr};

use super::{
    callback_url::GrpcCallbackUrl, grpc_callback_service::GrpcCallbackService,
};

#[derive(Debug)]
struct Inner {
    grpc: HashMap<GrpcCallbackUrl, Addr<GrpcCallbackService>>,
}

#[derive(Debug, Clone)]
pub struct CallbackRepository(Arc<Mutex<Inner>>);

impl CallbackRepository {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Inner {
            grpc: HashMap::new(),
        })))
    }

    pub fn get_grpc(
        &self,
        addr: &GrpcCallbackUrl,
    ) -> Addr<GrpcCallbackService> {
        let mut locked_self = self.0.lock().unwrap();
        if let Some(grpc_service) = locked_self.grpc.get(addr) {
            grpc_service.clone()
        } else {
            let grpc_service = GrpcCallbackService::new(addr).start();
            locked_self.grpc.insert(addr.clone(), grpc_service.clone());
            grpc_service
        }
    }
}
