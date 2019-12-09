//! Implementation of Control API gRPC [callback service].
//!
//! [callback service]: https://tinyurl.com/y5fajesq

use std::sync::{Arc, Mutex};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use clap::ArgMatches;
use futures::future::Future as _;
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};
use medea_control_api_proto::grpc::{
    callback::{Request, Response},
    callback_grpc::{create_callback, Callback as CallbackService},
};

use crate::{callback::CallbackItem, prelude::*};

/// Type which used in [`GrpcCallbackServer`] for [`CallbackItem`] storing.
type CallbackItems = Arc<Mutex<Vec<CallbackItem>>>;

/// [`Actor`] wrapper for [`grpcio`] server.
///
/// Also this [`Actor`] can return all received callbacks
/// with [`GetCallbacks`] [`Message`].
#[allow(clippy::module_name_repetitions)]
pub struct GrpcCallbackServer {
    /// [`grpcio`] gRPC server.
    server: Server,

    /// All [`Callback`]s which this server received.
    events: CallbackItems,
}

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.server.start();
    }
}

/// Implementation for [`CallbackService`] gRPC service.
#[derive(Clone)]
pub struct GrpcCallbackService {
    /// All [`Callback`]s which this server received.
    events: CallbackItems,
}

impl GrpcCallbackService {
    /// Returns [`GrpcCallbackService`] with provided pointer to [`Vec`] of
    /// [`CallbackItem`]s.
    pub fn new(events: CallbackItems) -> Self {
        Self { events }
    }
}

impl CallbackService for GrpcCallbackService {
    fn on_event(
        &mut self,
        ctx: RpcContext,
        req: Request,
        sink: UnarySink<Response>,
    ) {
        info!("Callback request received: [{:?}]", req);
        self.events.lock().unwrap().push(req.into());
        ctx.spawn(
            sink.success(Response::new())
                .map_err(|e| error!("Err: {:?}", e)),
        )
    }
}

/// [`Message`] which returns all [`Callback`]s received by this
/// [`GrpcCallbackServer`].
#[derive(Message)]
#[rtype(result = "Result<Vec<CallbackItem>, ()>")]
pub struct GetCallbackItems;

impl Handler<GetCallbackItems> for GrpcCallbackServer {
    type Result = Result<Vec<CallbackItem>, ()>;

    fn handle(
        &mut self,
        _: GetCallbackItems,
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

    let service =
        create_callback(GrpcCallbackService::new(Arc::clone(&events)));
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
