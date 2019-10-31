use actix::{Actor, Addr, Arbiter, Context, ResponseFuture};
use futures::future::Future as _;
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};
use medea_control_api_proto::grpc::{
    callback::{
        Request, Request_Event as RequestEventProto, Request_Event, Response,
    },
    callback_grpc::{create_callback, Callback as CallbackProto},
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize, Clone)]
pub struct Callback {
    element: String,
    event: CallbackEvent,
    at: String,
}

#[derive(Serialize, Clone)]
pub enum CallbackEvent {
    OnJoin,
    OnLeave,
}

impl From<RequestEventProto> for CallbackEvent {
    fn from(req: RequestEventProto) -> Self {
        match req {
            RequestEventProto::ON_JOIN => Self::OnJoin,
            RequestEventProto::ON_LEAVE => Self::OnLeave,
        }
    }
}

impl From<Request> for Callback {
    fn from(mut req: Request) -> Self {
        Self {
            element: req.take_element(),
            event: req.get_event().into(),
            at: req.take_at(),
        }
    }
}

pub struct GrpcCallbackServer(Server);

impl Actor for GrpcCallbackServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.0.start();
    }
}

#[derive(Clone)]
pub struct CallbackService {
    events: Vec<Callback>,
}

impl CallbackService {
    pub fn new() -> Self {
        Self { events: Vec::new() }
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
        self.events.push(req.into());
        ctx.spawn(
            sink.success(Response::new())
                .map_err(|e| println!("Err: {:?}", e)),
        )
    }
}

pub fn run() -> Addr<GrpcCallbackServer> {
    let cq_count = 2;

    let service = create_callback(CallbackService::new());
    let env = Arc::new(Environment::new(cq_count));

    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind("127.0.0.1", 9099)
        .build()
        .unwrap();

    GrpcCallbackServer::start_in_arbiter(&Arbiter::new(), move |_| {
        GrpcCallbackServer(server)
    })
}
