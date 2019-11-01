use std::sync::{Arc, Mutex};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use futures::future::Future as _;
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};
use medea_control_api_proto::grpc::{
    callback::{Request, Response},
    callback_grpc::{create_callback, Callback as CallbackProto},
};
use serde::Serialize;

use super::callback::Callback;

type Callbacks = Arc<Mutex<Vec<Callback>>>;

pub struct GrpcCallbackServer {
    server: Server,
    events: Callbacks,
}

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.server.start();
    }
}

#[derive(Clone)]
pub struct CallbackService {
    events: Callbacks,
}

impl CallbackService {
    pub fn new(events: Callbacks) -> Self {
        Self { events }
    }
}

impl CallbackProto for CallbackService {
    fn on_event(
        &mut self,
        ctx: RpcContext,
        req: Request,
        sink: UnarySink<Response>,
    ) {
        println!("Received event: {:#?}", req);
        self.events.lock().unwrap().push(req.into());
        ctx.spawn(
            sink.success(Response::new())
                .map_err(|e| println!("Err: {:?}", e)),
        )
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<Callback>, ()>")]
pub struct GetCallbacks;

impl Handler<GetCallbacks> for GrpcCallbackServer {
    type Result = Result<Vec<Callback>, ()>;

    fn handle(
        &mut self,
        _: GetCallbacks,
        _: &mut Self::Context,
    ) -> Self::Result {
        Ok(self.events.lock().unwrap().clone())
    }
}

pub fn run() -> Addr<GrpcCallbackServer> {
    let cq_count = 2;

    let events = Arc::new(Mutex::new(Vec::new()));

    let service = create_callback(CallbackService::new(Arc::clone(&events)));
    let env = Arc::new(Environment::new(cq_count));

    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind("127.0.0.1", 9099)
        .build()
        .unwrap();

    GrpcCallbackServer::start_in_arbiter(&Arbiter::new(), move |_| {
        GrpcCallbackServer { server, events }
    })
}
