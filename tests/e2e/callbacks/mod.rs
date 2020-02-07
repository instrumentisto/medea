//! Implementation of Control API callback server for tests.

mod member;

use std::sync::{Arc, Mutex};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use async_trait::async_trait;
use medea_control_api_proto::grpc::callback::{
    self as proto,
    callback_server::{Callback, CallbackServer as TonicCallbackServer},
};
use tonic::{transport::Server, Status};

/// Requests which [`GrpcCallbackServer`] will receive.
type CallbackItems = Arc<Mutex<Vec<proto::Request>>>;

/// gRPC Control API callback server for tests.
pub struct GrpcCallbackServer {
    callbacks: CallbackItems,
}

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;
}

/// Returns all [`proto::Request`]s which this [`GrpcCallbackServer`] received.
#[derive(Message)]
#[rtype(result = "Result<Vec<proto::Request>, ()>")]
pub struct GetCallbacks;

impl Handler<GetCallbacks> for GrpcCallbackServer {
    type Result = Result<Vec<proto::Request>, ()>;

    fn handle(
        &mut self,
        _: GetCallbacks,
        _: &mut Self::Context,
    ) -> Self::Result {
        Ok(self.callbacks.lock().unwrap().clone())
    }
}

/// [`tonic`] server for receiving callbacks.
#[derive(Clone)]
pub struct CallbackServer {
    callbacks: CallbackItems,
}

impl CallbackServer {
    pub fn new(callbacks: CallbackItems) -> Self {
        Self { callbacks }
    }
}

#[async_trait]
impl Callback for CallbackServer {
    async fn on_event(
        &self,
        request: tonic::Request<proto::Request>,
    ) -> Result<tonic::Response<proto::Response>, Status> {
        self.callbacks.lock().unwrap().push(request.into_inner());

        Ok(tonic::Response::new(proto::Response {}))
    }
}

/// Runs [`GrpcCallbackServer`] on `localhost` and provided port.
pub fn run(port: u16) -> Addr<GrpcCallbackServer> {
    let callbacks = Arc::new(Mutex::new(Vec::new()));

    let service =
        TonicCallbackServer::new(CallbackServer::new(Arc::clone(&callbacks)));

    Arbiter::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve(format!("127.0.0.1:{}", port).parse().unwrap())
            .await
            .unwrap()
    });

    GrpcCallbackServer::start_in_arbiter(&Arbiter::new(), move |_| {
        GrpcCallbackServer { callbacks }
    })
}
