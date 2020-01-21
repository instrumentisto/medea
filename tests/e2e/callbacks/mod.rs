//! Implementation of Control API callback server for tests.

mod member;

use std::sync::{Arc, Mutex};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use futures::{compat::Future01CompatExt, FutureExt, TryFutureExt};
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};
use medea_control_api_proto::grpc::{
    callback::{Request, Response},
    callback_grpc::{create_callback, Callback},
};

/// Requests which [`GrpcCallbackServer`] will receive.
type CallbackItems = Arc<Mutex<Vec<Request>>>;

/// gRPC Control API callback server for tests.
pub struct GrpcCallbackServer {
    server: Server,
    callbacks: CallbackItems,
}

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        self.server.start();
    }
}

/// Returns all [`Request`]s which this [`GrpcCallbackServer`] received.
#[derive(Message)]
#[rtype(result = "Result<Vec<Request>, ()>")]
pub struct GetCallbacks;

impl Handler<GetCallbacks> for GrpcCallbackServer {
    type Result = Result<Vec<Request>, ()>;

    fn handle(
        &mut self,
        _: GetCallbacks,
        _: &mut Self::Context,
    ) -> Self::Result {
        Ok(self.callbacks.lock().unwrap().clone())
    }
}

/// [`grpcio`] server for receiving callbacks.
#[derive(Clone)]
pub struct CallbackServer {
    callbacks: CallbackItems,
}

impl CallbackServer {
    pub fn new(callbacks: CallbackItems) -> Self {
        Self { callbacks }
    }
}

impl Callback for CallbackServer {
    fn on_event(
        &mut self,
        ctx: RpcContext,
        req: Request,
        sink: UnarySink<Response>,
    ) {
        self.callbacks.lock().unwrap().push(req);
        ctx.spawn(
            async move {
                sink.success(Response::new()).compat().await.unwrap();
                Ok(())
            }
            .boxed()
            .compat(),
        )
    }
}

/// Runs [`GrpcCallbackServer`] on `localhost` and provided port.
pub fn run(port: u16) -> Addr<GrpcCallbackServer> {
    let cq_count = 2;
    let callbacks = Arc::new(Mutex::new(Vec::new()));

    let service = create_callback(CallbackServer::new(Arc::clone(&callbacks)));
    let env = Arc::new(Environment::new(cq_count));

    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind("127.0.0.1", port)
        .build()
        .unwrap();

    GrpcCallbackServer::start_in_arbiter(&Arbiter::new(), move |_| {
        GrpcCallbackServer { server, callbacks }
    })
}
