//! Implementation of Control API callback server for tests.

mod endpoint;
mod member;

use std::sync::{Arc, Mutex};

use actix::{Actor, Addr, Arbiter, Context, Handler, Message};
use async_trait::async_trait;
use medea_control_api_proto::grpc::callback::{
    self as proto,
    callback_server::{Callback, CallbackServer as TonicCallbackServer},
};
use tonic::{transport::Server, Status};

mod test_ports {
    macro_rules! ports {
        ($($name:ident => $value:expr),* $(,)*) => {
            // This enum is needed for checking that all ports are unique.
            #[allow(dead_code, non_camel_case_types)]
            enum _CheckTestPorts {
                $($name = $value,)*
            }

            $(pub const $name: u16 = $value;)*
        };
    }

    ports! {
        MEMBER_ON_JOIN => 9096,
        MEMBER_ON_LEAVE_NORMALLY_DISCONNECTED => 9097,
        MEMBER_ON_LEAVE_ON_CONNECTION_LOSS => 9098,
        ENDPOINT_ON_START_WORKS => 9099,
        ENDPOINT_ON_STOP_WORKS_ON_LEAVE => 9100,
        ENDPOINT_ON_STOP_BY_TIMEOUT => 9101,
        ENDPOINT_ON_STOP_ON_CONTRADICTION => 9102,
        ENDPOINT_ON_STOP_DIDNT_FIRES_WHILE_ALL_NORMAL => 9103,
    }
}

/// Requests which [`GrpcCallbackServer`] will receive.
type CallbackItems = Arc<Mutex<Vec<proto::Request>>>;

/// gRPC Control API callback server for tests.
pub struct GrpcCallbackServer {
    callbacks: CallbackItems,
}

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;
}

pub struct Callbacks(pub Vec<proto::Request>);

macro_rules! gen_event_filter_fn {
    ($name:tt -> $event:path) => {
        pub fn $name(&self) -> impl Iterator<Item = &proto::Request> {
            self.0.iter().filter(|req| {
                if let Some($event(_)) = req.event {
                    true
                } else {
                    false
                }
            })
        }
    };
}

impl Callbacks {
    gen_event_filter_fn!(get_on_starts -> proto::request::Event::OnStart);

    gen_event_filter_fn!(get_on_stops -> proto::request::Event::OnStop);

    gen_event_filter_fn!(get_on_joins -> proto::request::Event::OnJoin);
}

/// Returns all [`proto::Request`]s which this [`GrpcCallbackServer`] received.
#[derive(Message)]
#[rtype(result = "Result<Callbacks, ()>")]
pub struct GetCallbacks;

impl Handler<GetCallbacks> for GrpcCallbackServer {
    type Result = Result<Callbacks, ()>;

    fn handle(
        &mut self,
        _: GetCallbacks,
        _: &mut Self::Context,
    ) -> Self::Result {
        Ok(Callbacks(self.callbacks.lock().unwrap().clone()))
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
