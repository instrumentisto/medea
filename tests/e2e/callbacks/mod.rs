mod member;

use std::sync::{Arc, Mutex};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use futures::Future as _;
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};
use medea_control_api_proto::grpc::{
    callback::{Request},
    callback_grpc::{create_callback, Callback},
};
use protobuf::well_known_types::Empty;

type Callbacks = Arc<Mutex<Vec<Request>>>;

pub struct GrpcCallbackServer {
    server: Server,
    callbacks: Callbacks,
}

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        self.server.start();
    }
}

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

#[derive(Clone)]
pub struct CallbackService {
    callbacks: Callbacks,
}

impl CallbackService {
    pub fn new(callbacks: Callbacks) -> Self {
        Self { callbacks }
    }
}

impl Callback for CallbackService {
    fn on_event(
        &mut self,
        ctx: RpcContext,
        req: Request,
        sink: UnarySink<Empty>,
    ) {
        self.callbacks.lock().unwrap().push(req);
        ctx.spawn(sink.success(Empty::new()).map_err(|e| panic!("{:?}", e)))
    }
}

pub fn run(port: u16) -> Addr<GrpcCallbackServer> {
    let cq_count = 2;
    let callbacks = Arc::new(Mutex::new(Vec::new()));

    let service = create_callback(CallbackService::new(Arc::clone(&callbacks)));
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
