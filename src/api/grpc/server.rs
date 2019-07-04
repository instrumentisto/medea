use std::{sync::Arc, time::Duration};

use actix::{Actor, Addr, Arbiter, Context};
use futures::future::Future;
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};

use crate::{
    api::{
        control::{
            model::{room::RoomSpec, RoomId},
            protobuf::room::CreateRequestSpec,
        },
        grpc::protos::control::{
            ApplyRequest, CreateRequest, GetResponse, IdRequest, Response,
        },
    },
    conf::Conf,
    log::prelude::*,
    signalling::{room_repo::RoomsRepository, Room},
};

use super::protos::control_grpc::{create_control_api, ControlApi};

#[derive(Clone)]
struct ControlApiService {
    room_repository: RoomsRepository,
    config: Conf,
}

impl ControlApi for ControlApiService {
    fn create(
        &mut self,
        _ctx: RpcContext,
        req: CreateRequest,
        _sink: UnarySink<Response>,
    ) {
        // TODO
        let room_id = RoomId(req.get_id().to_string());

        let room = Room::start_in_arbiter(&Arbiter::new(), |_| {
            let room_spec = CreateRequestSpec(req);
            let room_spec = Box::new(&room_spec as &RoomSpec);

            let turn_auth_service =
                crate::turn::service::new_turn_auth_service(&Conf::default())
                    .expect("Unable to start turn service");
            Room::new(&room_spec, Duration::from_secs(10), turn_auth_service)
                .unwrap()
        });

        self.room_repository.add(room_id, room);

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
        self.server.start();
        debug!("gRPC server started.");
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("Shutdown gRPC.");
        self.server.shutdown().wait().unwrap();
    }
}

pub fn run(room_repo: RoomsRepository, conf: Conf) -> Addr<GrpcServer> {
    let bind_ip = conf.grpc.bind_ip.clone().to_string();
    let bind_port = conf.grpc.bind_port;
    let cq_count = conf.grpc.completion_queue_count;

    let service = create_control_api(ControlApiService {
        config: conf,
        room_repository: room_repo,
    });
    let env = Arc::new(Environment::new(cq_count));

    info!("Starting gRPC server on {}:{}", bind_ip, bind_port);

    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind(bind_ip, bind_port)
        .build()
        .unwrap();

    GrpcServer::start_in_arbiter(&Arbiter::new(), move |_| GrpcServer {
        server,
    })
}
