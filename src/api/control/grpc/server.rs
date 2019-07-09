use std::{collections::HashMap, sync::Arc};

use actix::{Actor, Addr, Arbiter, Context};
use failure::Fail;
use futures::future::{Either, Future};
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};

use crate::{
    api::control::{
        grpc::protos::control::{
            ApplyRequest, CreateRequest, Error, GetResponse, IdRequest,
            Response,
        },
        local_uri::{LocalUri, LocalUriParseError},
        RoomSpec, TryFromElementError, TryFromProtobufError,
    },
    log::prelude::*,
    signalling::{
        room::RoomError,
        room_repo::{
            DeleteEndpointFromMember, DeleteMemberFromRoom, DeleteRoom,
            GetRoom, RoomsRepository, StartRoom,
        },
    },
    App,
};

use super::protos::control_grpc::{create_control_api, ControlApi};
use crate::signalling::room_repo::GetMember;

#[derive(Debug, Fail)]
enum ControlApiError {
    #[fail(display = "{:?}", _0)]
    LocalUri(LocalUriParseError),
    #[fail(display = "{:?}", _0)]
    TryFromProtobuf(TryFromProtobufError),
    #[fail(display = "{:?}", _0)]
    TryFromElement(TryFromElementError),
}

impl From<LocalUriParseError> for ControlApiError {
    fn from(from: LocalUriParseError) -> Self {
        ControlApiError::LocalUri(from)
    }
}

impl From<TryFromProtobufError> for ControlApiError {
    fn from(from: TryFromProtobufError) -> Self {
        ControlApiError::TryFromProtobuf(from)
    }
}

impl From<TryFromElementError> for ControlApiError {
    fn from(from: TryFromElementError) -> Self {
        ControlApiError::TryFromElement(from)
    }
}

#[derive(Clone)]
struct ControlApiService {
    room_repository: Addr<RoomsRepository>,
    app: Arc<App>,
}

impl ControlApiService {
    pub fn create_room(
        &mut self,
        req: CreateRequest,
    ) -> Result<
        impl Future<Item = HashMap<String, String>, Error = ()>,
        ControlApiError,
    > {
        let local_uri = LocalUri::parse(req.get_id())?;
        let room_id = local_uri.room_id.unwrap();

        let room =
            RoomSpec::try_from_protobuf(room_id.clone(), req.get_room())?;

        let sid: HashMap<String, String> = room
            .members()?
            .iter()
            .map(|(id, member)| {
                let addr = &self.app.config.server.bind_ip;
                let port = self.app.config.server.bind_port;
                let base_uri = format!("{}:{}", addr, port);

                let uri = format!(
                    "wss://{}/{}/{}/{}",
                    base_uri,
                    &room_id,
                    id,
                    member.credentials()
                );

                (id.clone().to_string(), uri)
            })
            .collect();

        // TODO: errors from `SendRoom` not bubbled up.
        Ok(self
            .room_repository
            .send(StartRoom(room_id, room))
            .map_err(|e| error!("Start room mailbox error. {:?}", e))
            .map(move |_| sid))
    }
}

impl ControlApi for ControlApiService {
    fn create(
        &mut self,
        ctx: RpcContext,
        req: CreateRequest,
        sink: UnarySink<Response>,
    ) {
        let local_uri = LocalUri::parse(req.get_id()).unwrap();

        let create = {
            if local_uri.is_room_uri() {
                self.create_room(req)
            } else {
                unimplemented!()
            }
        };

        match create {
            Ok(fut) => ctx.spawn(fut.and_then(move |r| {
                let mut response = Response::new();
                response.set_sid(r);
                sink.success(response).map_err(|_| ())
            })),
            Err(e) => {
                let mut res = Response::new();
                let mut error = Error::new();

                match e {
                    ControlApiError::TryFromProtobuf(e) => match e {
                        TryFromProtobufError::MemberElementNotFound
                        | TryFromProtobufError::MemberCredentialsNotFound
                        | TryFromProtobufError::P2pModeNotFound
                        | TryFromProtobufError::SrcUriNotFound
                        | TryFromProtobufError::RoomElementNotFound => {
                            error.set_status(404);
                            error.set_code(0);
                            error.set_text(e.to_string());
                            error.set_element(String::new());
                        }
                        TryFromProtobufError::SrcUriError(e) => {
                            error.set_status(400);
                            error.set_code(0);
                            error.set_text(e.to_string());
                            error.set_element(String::new());
                        }
                    },
                    ControlApiError::TryFromElement(e) => {
                        error.set_status(400);
                        error.set_code(0);
                        error.set_text(e.to_string());
                        error.set_element(String::new());
                    }
                    ControlApiError::LocalUri(e) => {
                        error.set_status(400);
                        error.set_code(0);
                        error.set_text(e.to_string());
                        error.set_element(String::new());
                    }
                }

                res.set_error(error);
                ctx.spawn(sink.success(res).map_err(|_| ()));
            }
        }
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
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<Response>,
    ) {
        for id in req.get_id() {
            let uri = LocalUri::parse(id).unwrap(); // TODO

            if uri.is_room_uri() {
                self.room_repository
                    .do_send(DeleteRoom(uri.room_id.unwrap()));
            } else if uri.is_member_uri() {
                self.room_repository.do_send(DeleteMemberFromRoom {
                    room_id: uri.room_id.unwrap(),
                    member_id: uri.member_id.unwrap(),
                });
            } else if uri.is_endpoint_uri() {
                self.room_repository.do_send(DeleteEndpointFromMember {
                    room_id: uri.room_id.unwrap(),
                    member_id: uri.member_id.unwrap(),
                    endpoint_id: uri.endpoint_id.unwrap(),
                });
            }
        }

        let mut resp = Response::new();
        resp.set_sid(HashMap::new());
        ctx.spawn(
            sink.success(resp)
                .map_err(|e| error!("gRPC response failed. {:?}", e)),
        );
    }

    fn get(
        &mut self,
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<GetResponse>,
    ) {
        let mut room_ids = Vec::new();

        for uri in req.get_id() {
            let local_uri = LocalUri::parse(uri).unwrap();
            room_ids.push((
                local_uri.room_id.unwrap(),
                local_uri.member_id.unwrap(),
            ));
        }

        ctx.spawn(
            self.room_repository
                .send(GetMember(room_ids))
                .map_err(|e| ())
                .and_then(|r| {
                    let result = r.unwrap();

                    let elements = result.into_iter().collect();

                    let mut response = GetResponse::new();
                    response.set_elements(elements);

                    sink.success(response).map_err(|_| ())
                }),
        );
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

pub fn run(
    room_repo: Addr<RoomsRepository>,
    app: Arc<App>,
) -> Addr<GrpcServer> {
    let bind_ip = app.config.grpc.bind_ip.clone().to_string();
    let bind_port = app.config.grpc.bind_port;
    let cq_count = app.config.grpc.completion_queue_count;

    let service = create_control_api(ControlApiService {
        app: app,
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
