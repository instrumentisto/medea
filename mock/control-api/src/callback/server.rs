//! Implementation of Control API gRPC [Callback service].
//!
//! [Callback service]: https://tinyurl.com/y5fajesq

use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use clap::ArgMatches;
use medea_control_api_proto::grpc::callback::{
    self as proto,
    callback_server::{
        Callback as CallbackService, CallbackServer as TonicCallbackServer,
    },
};
use tonic::transport::Server;

use crate::{callback::CallbackItem, prelude::*};

/// Type which used in [`GrpcCallbackServer`] for [`CallbackItem`] storing.
type CallbackItems = Arc<Mutex<Vec<CallbackItem>>>;

/// [`Actor`] wrapper for [`tonic`] gRPC server.
///
/// Also this [`Actor`] can return all received callbacks
/// with [`GetCallbackItems`] [`Message`].
pub struct GrpcCallbackServer {
    /// All [`CallbackItem`]s which this server received.
    events: CallbackItems,
}

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;
}

/// Implementation for [`CallbackService`] gRPC service.
#[derive(Clone)]
pub struct GrpcCallbackService {
    /// All [`CallbackItem`]s which this server received.
    events: CallbackItems,
}

impl GrpcCallbackService {
    /// Returns [`GrpcCallbackService`] with provided pointer to [`Vec`] of
    /// [`CallbackItem`]s.
    #[must_use]
    pub fn new(events: CallbackItems) -> Self {
        Self { events }
    }
}

#[tonic::async_trait]
impl CallbackService for GrpcCallbackService {
    async fn on_event(
        &self,
        req: tonic::Request<proto::Request>,
    ) -> Result<tonic::Response<proto::Response>, tonic::Status> {
        info!("Callback request received: [{:?}]", req);
        self.events.lock().unwrap().push(req.into_inner().into());
        Ok(tonic::Response::new(proto::Response {}))
    }
}

/// [`Message`] which returns all [`CallbackItem`]s received by this
/// [`GrpcCallbackServer`].
#[derive(Message)]
#[rtype(result = "Result<Vec<CallbackItem>, Infallible>")]
pub struct GetCallbackItems;

impl Handler<GetCallbackItems> for GrpcCallbackServer {
    type Result = Result<Vec<CallbackItem>, Infallible>;

    fn handle(
        &mut self,
        _: GetCallbackItems,
        _: &mut Self::Context,
    ) -> Self::Result {
        Ok(self.events.lock().unwrap().clone())
    }
}

/// Run [`GrpcCallbackServer`].
///
/// # Panics
///
/// If the given `args` don't contain expected `callback_host` and
/// `callback_port` values.
pub async fn run(args: &ArgMatches<'static>) -> Addr<GrpcCallbackServer> {
    let host = args.value_of("callback_host").unwrap();
    let port: u32 = args.value_of("callback_port").unwrap().parse().unwrap();

    let events = Arc::new(Mutex::new(Vec::new()));

    let service =
        TonicCallbackServer::new(GrpcCallbackService::new(Arc::clone(&events)));
    let addr = format!("{}:{}", host, port).parse().unwrap();

    Arbiter::current().spawn(async move {
        Server::builder()
            .add_service(service)
            .serve(addr)
            .await
            .unwrap();
    });

    debug!("gRPC callback server started.");

    GrpcCallbackServer::start_in_arbiter(&Arbiter::new().handle(), move |_| {
        GrpcCallbackServer { events }
    })
}
