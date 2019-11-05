//! Implementation of Control API gRPC [callback service].
//!
//! [callback service]: https://tinyurl.com/y5fajesq

use std::sync::{Arc, Mutex};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use futures::future::Future as _;
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};
use medea_control_api_proto::grpc::{
    callback::{Request, Response},
    callback_grpc::{create_callback, Callback as CallbackProto},
};

use super::Callback;
use clap::ArgMatches;

type Callbacks = Arc<Mutex<Vec<Callback>>>;

/// [`Actor`] wrapper for [`grpcio`] server.
///
/// Also this [`Actor`] can return all received callbacks
/// with [`GetCallbacks`] [`Message`].
pub struct GrpcCallbackServer {
    /// [`grpcio`] gRPC server.
    server: Server,

    /// All [`Callback`]s which this server received.
    events: Callbacks,
}

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.server.start();
    }
}

/// Implementation for [`CallbackProto`] gRPC service.
#[derive(Clone)]
pub struct CallbackService {
    /// All [`Callback`]s which this server received.
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
        self.events.lock().unwrap().push(req.into());
        ctx.spawn(
            sink.success(Response::new())
                .map_err(|e| println!("Err: {:?}", e)),
        )
    }
}

/// [`Message`] which returns all [`Callback`]s received by this
/// [`GrpcCallbackServer`].
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

/// Run [`GrpcCallbackServer`].
pub fn run(args: &ArgMatches) -> Addr<GrpcCallbackServer> {
    let host = args.value_of("callback_host").unwrap();
    let port = args.value_of("callback_port").unwrap().parse().unwrap();
    let cq_count = 2;

    let events = Arc::new(Mutex::new(Vec::new()));

    let service = create_callback(CallbackService::new(Arc::clone(&events)));
    let env = Arc::new(Environment::new(cq_count));

    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind(host, port)
        .build()
        .unwrap();

    GrpcCallbackServer::start_in_arbiter(&Arbiter::new(), move |_| {
        GrpcCallbackServer { server, events }
    })
}
