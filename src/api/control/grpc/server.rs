//! Implementation of gRPC control API.

use std::{collections::HashMap, convert::TryFrom, sync::Arc};

use actix::{Actor, Addr, Arbiter, Context, MailboxError};
use failure::Fail;
use futures::future::{Either, Future};
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};

use crate::{
    api::{
        control::{
            grpc::protos::control::{
                ApplyRequest, CreateRequest, Error, GetResponse, IdRequest,
                Response,
            },
            local_uri::{LocalUri, LocalUriParseError},
            Endpoint, MemberSpec, RoomSpec, TryFromElementError,
            TryFromProtobufError,
        },
        error_codes::ErrorCode,
    },
    log::prelude::*,
    signalling::{
        room::RoomError,
        room_repo::{
            CreateEndpointInRoom, CreateMemberInRoom, DeleteEndpointFromMember,
            DeleteMemberFromRoom, DeleteRoom, GetEndpoint, GetMember, GetRoom,
            RoomRepoError, RoomsRepository, StartRoom,
        },
    },
    App,
};

use super::protos::control_grpc::{create_control_api, ControlApi};

#[derive(Debug, Fail)]
enum ControlApiError {
    /// Error when parsing ID of element.
    #[fail(display = "{:?}", _0)]
    LocalUri(LocalUriParseError),

    /// This error is rather abnormal, since what it catches must be caught at
    /// the level of the gRPC.
    #[fail(display = "{:?}", _0)]
    TryFromProtobuf(TryFromProtobufError),

    /// This error is rather abnormal, since what it catches must be caught at
    /// the level of the gRPC.
    #[fail(display = "{:?}", _0)]
    TryFromElement(TryFromElementError),

    /// Wrapped [`MailboxError`].
    #[fail(display = "{:?}", _0)]
    MailboxError(MailboxError),
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

impl From<MailboxError> for ControlApiError {
    fn from(from: MailboxError) -> Self {
        ControlApiError::MailboxError(from)
    }
}

impl Into<ErrorCode> for ControlApiError {
    fn into(self) -> ErrorCode {
        match self {
            ControlApiError::LocalUri(e) => e.into(),
            ControlApiError::TryFromProtobuf(e) => e.into(),
            _ => ErrorCode::UnknownError(self.to_string()),
        }
    }
}

macro_rules! fut_try {
    ($call:expr) => {
        match $call {
            Ok(o) => o,
            Err(e) => {
                return Either::B(futures::future::err(ControlApiError::from(
                    e,
                )))
            }
        }
    };
}

macro_rules! parse_local_uri {
    ($uri:expr, $ctx:expr, $sink:expr, $response:ty) => {
        match LocalUri::parse($uri) {
            Ok(o) => o,
            Err(e) => {
                let mut error_response = <$response>::new();
                let mut error = Error::new();
                error.set_status(400);
                error.set_code(0);
                error.set_text(format!("Invalid ID [id = {}]. {}", $uri, e));
                error.set_element($uri.to_string());
                error_response.set_error(error);
                $ctx.spawn($sink.success(error_response).map_err(|_| ()));
                return;
            }
        }
    };
}

#[derive(Clone)]
struct ControlApiService {
    room_repository: Addr<RoomsRepository>,
    app: Arc<App>,
}

impl ControlApiService {
    /// Implementation of `Create` method for `Room` element.
    pub fn create_room(
        &mut self,
        req: CreateRequest,
        local_uri: LocalUri,
    ) -> impl Future<
        Item = Result<
            Result<HashMap<String, String>, RoomError>,
            RoomRepoError,
        >,
        Error = ControlApiError,
    > {
        let room_id = local_uri.room_id.unwrap();

        let room = fut_try!(RoomSpec::try_from_protobuf(
            room_id.clone(),
            req.get_room()
        ));

        let sid: HashMap<String, String> = fut_try!(room.members())
            .iter()
            .map(|(id, member)| {
                let base_url = self.app.config.get_base_rpc_url();

                let uri = format!(
                    "{}/{}/{}/{}",
                    base_url,
                    &room_id,
                    id,
                    member.credentials()
                );

                (id.clone().to_string(), uri)
            })
            .collect();

        Either::A(
            self.room_repository
                .send(StartRoom(room_id, room))
                .map_err(|e| ControlApiError::from(e))
                .map(move |r| r.map(|_| Ok(sid))),
        )
    }

    /// Implementation of `Create` method for `Member` element.
    pub fn create_member(
        &mut self,
        req: CreateRequest,
        local_uri: LocalUri,
    ) -> impl Future<
        Item = Result<
            Result<HashMap<String, String>, RoomError>,
            RoomRepoError,
        >,
        Error = ControlApiError,
    > {
        let spec = fut_try!(MemberSpec::try_from(req.get_member()));

        let room_id = local_uri.room_id.unwrap();
        let member_id = local_uri.member_id.unwrap();

        let base_url = self.app.config.get_base_rpc_url();
        let sid = format!(
            "{}/{}/{}/{}",
            base_url,
            room_id,
            member_id,
            spec.credentials()
        );
        let mut sids = HashMap::new();
        sids.insert(member_id.to_string(), sid);

        Either::A(
            self.room_repository
                .send(CreateMemberInRoom {
                    room_id,
                    member_id,
                    spec,
                })
                .map_err(|e| ControlApiError::from(e))
                .map(|r| r.map(|r| r.map(|_| sids))),
        )
    }

    /// Implementation of `Create` method for `WebRtcPublishEndpoint` and
    /// `WebRtcPlayEndpoint` elements.
    pub fn create_endpoint(
        &mut self,
        req: CreateRequest,
        local_uri: LocalUri,
    ) -> impl Future<
        Item = Result<
            Result<HashMap<String, String>, RoomError>,
            RoomRepoError,
        >,
        Error = ControlApiError,
    > {
        let endpoint = fut_try!(Endpoint::try_from(&req));
        Either::A(
            self.room_repository
                .send(CreateEndpointInRoom {
                    room_id: local_uri.room_id.unwrap(),
                    member_id: local_uri.member_id.unwrap(),
                    endpoint_id: local_uri.endpoint_id.unwrap(),
                    spec: endpoint,
                })
                .map_err(|e| ControlApiError::from(e))
                .map(|r| r.map(|r| r.map(|_| HashMap::new()))),
        )
    }
}

/// Generate [`Response`] for `Create` method of all elements.
fn create_response(
    result: Result<
        Result<Result<HashMap<String, String>, RoomError>, RoomRepoError>,
        ControlApiError,
    >,
) -> Response {
    let error: ErrorCode = match result {
        Ok(r) => match r {
            Ok(r) => match r {
                Ok(sid) => {
                    let mut response = Response::new();
                    response.set_sid(sid);
                    return response;
                }
                Err(e) => e.into(),
            },
            Err(e) => e.into(),
        },
        Err(e) => e.into(),
    };

    let mut error_response = Response::new();
    error_response.set_error(error.into());
    error_response
}

fn error_response(
    sink: UnarySink<Response>,
    error_code: ErrorCode,
) -> impl Future<Item = (), Error = ()> {
    let mut response = Response::new();
    let error: Error = error_code.into();
    response.set_error(error);

    sink.success(response).map_err(|_| ())
}

impl ControlApi for ControlApiService {
    /// Implementation for `Create` method of gRPC control API.
    fn create(
        &mut self,
        ctx: RpcContext,
        req: CreateRequest,
        sink: UnarySink<Response>,
    ) {
        let local_uri = parse_local_uri!(req.get_id(), ctx, sink, Response);

        if local_uri.is_room_uri() {
            if req.has_room() {
                ctx.spawn(self.create_room(req, local_uri).then(move |r| {
                    sink.success(create_response(r)).map_err(|_| ())
                }));
            } else {
                ctx.spawn(error_response(
                    sink,
                    ErrorCode::ElementIdForRoomButElementIsNot(
                        req.get_id().to_string(),
                    ),
                ));
            }
        } else if local_uri.is_member_uri() {
            if req.has_member() {
                ctx.spawn(self.create_member(req, local_uri).then(move |r| {
                    sink.success(create_response(r)).map_err(|_| ())
                }));
            } else {
                ctx.spawn(error_response(
                    sink,
                    ErrorCode::ElementIdForMemberButElementIsNot(
                        req.get_id().to_string(),
                    ),
                ));
            }
        } else if local_uri.is_endpoint_uri() {
            if req.has_webrtc_pub() || req.has_webrtc_play() {
                ctx.spawn(self.create_endpoint(req, local_uri).then(
                    move |r| sink.success(create_response(r)).map_err(|_| ()),
                ));
            } else {
                ctx.spawn(error_response(
                    sink,
                    ErrorCode::ElementIdForEndpointButElementIsNot(
                        req.get_id().to_string(),
                    ),
                ));
            }
        } else {
            ctx.spawn(error_response(
                sink,
                ErrorCode::InvalidElementUri(req.get_id().to_string()),
            ));
        }
    }

    /// Implementation for `Apply` method of gRPC control API.
    fn apply(
        &mut self,
        _ctx: RpcContext,
        _req: ApplyRequest,
        _sink: UnarySink<Response>,
    ) {
        unimplemented!()
    }

    /// Implementation for `Delete` method of gRPC control API.
    fn delete(
        &mut self,
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<Response>,
    ) {
        let mut delete_room_futs = Vec::new();
        let mut delete_member_futs = Vec::new();
        let mut delete_endpoints_futs = Vec::new();

        for id in req.get_id() {
            let uri = parse_local_uri!(id, ctx, sink, Response);

            if uri.is_room_uri() {
                delete_room_futs.push(
                    self.room_repository.send(DeleteRoom(uri.room_id.unwrap())),
                );
            } else if uri.is_member_uri() {
                delete_member_futs.push(self.room_repository.send(
                    DeleteMemberFromRoom {
                        room_id: uri.room_id.unwrap(),
                        member_id: uri.member_id.unwrap(),
                    },
                ));
            } else if uri.is_endpoint_uri() {
                delete_endpoints_futs.push(self.room_repository.send(
                    DeleteEndpointFromMember {
                        room_id: uri.room_id.unwrap(),
                        member_id: uri.member_id.unwrap(),
                        endpoint_id: uri.endpoint_id.unwrap(),
                    },
                ));
            }
        }

        let mega_delete_room_fut = futures::future::join_all(delete_room_futs);
        let mega_delete_member_fut =
            futures::future::join_all(delete_member_futs);
        let mega_delete_endpoints_fut =
            futures::future::join_all(delete_endpoints_futs);

        ctx.spawn(
            mega_delete_endpoints_fut
                .join3(mega_delete_member_fut, mega_delete_room_fut)
                .map_err(|_| ())
                .and_then(move |(member, endpoint, room)| {
                    member
                        .into_iter()
                        .chain(endpoint.into_iter())
                        .chain(room.into_iter())
                        .for_each(|r| r.unwrap());
                    // TODO
                    let mut response = Response::new();
                    response.set_sid(HashMap::new());
                    sink.success(response).map_err(|_| ())
                }),
        );
    }

    /// Implementation for `Get` method of gRPC control API.
    fn get(
        &mut self,
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<GetResponse>,
    ) {
        let mut room_ids = Vec::new();
        let mut member_ids = Vec::new();
        let mut endpoint_ids = Vec::new();

        for id in req.get_id() {
            let local_uri = parse_local_uri!(id, ctx, sink, GetResponse);

            if local_uri.is_room_uri() {
                room_ids.push(local_uri.room_id.unwrap());
            } else if local_uri.is_member_uri() {
                member_ids.push((
                    local_uri.room_id.unwrap(),
                    local_uri.member_id.unwrap(),
                ));
            } else if local_uri.is_endpoint_uri() {
                endpoint_ids.push((
                    local_uri.room_id.unwrap(),
                    local_uri.member_id.unwrap(),
                    local_uri.endpoint_id.unwrap(),
                ));
            }
        }

        let room_fut = self.room_repository.send(GetRoom(room_ids));
        let member_fut = self.room_repository.send(GetMember(member_ids));
        let endpoint_fut = self.room_repository.send(GetEndpoint(endpoint_ids));

        let mega_future = room_fut
            .join3(member_fut, endpoint_fut)
            .map_err(|e| println!("{:?}", e))
            .and_then(|(room, member, endpoint)| {
                let mut elements = HashMap::new();
                let mut elements_results = Vec::new();

                let results = vec![room, member, endpoint];

                let closure = |_| ();

                for result in results {
                    match result {
                        Ok(o) => {
                            elements_results.push(o);
                        }
                        Err(e) => {
                            let mut response = GetResponse::new();
                            let error: ErrorCode = e.into();
                            response.set_error(error.into());
                            return sink.success(response).map_err(closure);
                        }
                    }
                }

                let elements_results =
                    elements_results.into_iter().flat_map(|e| e.into_iter());

                for element in elements_results {
                    match element {
                        Ok((id, o)) => {
                            elements.insert(id, o);
                        }
                        Err(e) => {
                            let mut response = GetResponse::new();
                            let error: ErrorCode = e.into();
                            response.set_error(error.into());
                            return sink.success(response).map_err(closure);
                        }
                    }
                }

                let mut response = GetResponse::new();
                response.set_elements(elements);

                sink.success(response).map_err(closure)
            });

        ctx.spawn(mega_future);
    }
}

/// Actor wrapper for `grcio` gRPC server.
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

/// Run gRPC server in actix actor.
pub fn run(
    room_repo: Addr<RoomsRepository>,
    app: Arc<App>,
) -> Addr<GrpcServer> {
    let bind_ip = app.config.grpc.bind_ip.clone().to_string();
    let bind_port = app.config.grpc.bind_port;
    let cq_count = app.config.grpc.completion_queue_count;

    let service = create_control_api(ControlApiService {
        app,
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
