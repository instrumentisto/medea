use std::sync::Arc;

use actix::{Actor, Addr, Arbiter, Context};
use futures::future::Future;
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};

use crate::{
    api::{
        control::model::RoomId,
        grpc::protos::control::{
            ApplyRequest, CreateRequest, GetResponse, IdRequest, Response,
        },
    },
    log::prelude::*,
    signalling::room_repo::RoomsRepository,
};

use super::protos::control_grpc::{create_control_api, ControlApi};

#[derive(Clone)]
struct ControlApiService {
    room_repository: RoomsRepository,
}

impl ControlApi for ControlApiService {
    fn create(
        &mut self,
        _ctx: RpcContext,
        _req: CreateRequest,
        _sink: UnarySink<Response>,
    ) {
        self.room_repository
            .remove(&RoomId("pub-sub-video-call".to_string()));
        debug!("{:?}", self.room_repository);
    }

    fn apply(
        &mut self,
        _ctx: RpcContext,
        _req: ApplyRequest,
        _sink: UnarySink<Response>,
    ) {
        unimplemented!()
    }

    fn delete(
        &mut self,
        _ctx: RpcContext,
        _req: IdRequest,
        _sink: UnarySink<Response>,
    ) {
        unimplemented!()
    }

    fn get(
        &mut self,
        _ctx: RpcContext,
        _req: IdRequest,
        _sink: UnarySink<GetResponse>,
    ) {
        unimplemented!()
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct GrpcServer {
    server: Server,
}

impl Actor for GrpcServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        debug!("Start gRPC server.");
        self.server.start();
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("Shutdown gRPC.");
        self.server.shutdown().wait().unwrap();
    }
}

pub fn run(room_repo: RoomsRepository) -> Addr<GrpcServer> {
    let service = create_control_api(ControlApiService {
        room_repository: room_repo,
    });
    let env = Arc::new(Environment::new(1));

    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind("127.0.0.1", 50_051)
        .build()
        .unwrap();

    GrpcServer::start_in_arbiter(&Arbiter::new(), move |_| GrpcServer {
        server,
    })
}
