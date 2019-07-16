//! Implementation of gRPC control API.

// Fix clippy needless_return in macro.
#![allow(clippy::needless_return)]

use std::{collections::HashMap, convert::TryFrom, sync::Arc};

use actix::{Actor, Addr, Arbiter, Context, MailboxError};
use failure::Fail;
use futures::future::{self, Either, Future};
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};

use crate::{
    api::{
        control::{
            grpc::protos::control::{
                ApplyRequest, CreateRequest, Error, GetResponse, IdRequest,
                Response,
            },
            local_uri::{LocalUri, LocalUriParseError, LocalUriType},
            Endpoint, MemberSpec, RoomSpec, TryFromElementError,
            TryFromProtobufError,
        },
        error_codes::ErrorCode,
    },
    log::prelude::*,
    signalling::{
        room::RoomError,
        room_service::{
            CreateEndpointInRoom, CreateMemberInRoom, DeleteEndpointFromMember,
            DeleteMemberFromRoom, DeleteRoom, GetEndpoint, GetMember, GetRoom,
            RoomService, RoomServiceError, StartRoom,
        },
    },
    AppContext,
};

use super::protos::control_grpc::{create_control_api, ControlApi};
use crate::api::control::local_uri::{IsEndpointId, IsMemberId, IsRoomId};

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

/// Try to unwrap some [`Result`] and if it `Err` then return err future with
/// [`ControlApiError`].
///
/// __Note:__ this macro returns [`Either::B`].
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

/// Macro for parse [`LocalUri`] and send error to client if some error occurs.
///
/// See `send_error_response` doc for details about arguments for this macro.
macro_rules! parse_local_uri {
    ($uri:expr, $ctx:expr, $sink:expr, $response:ty) => {
        match LocalUriType::parse($uri) {
            Ok(o) => o,
            Err(e) => {
                let error: ErrorCode = e.into();
                send_error_response!($ctx, $sink, error, $response);
            }
        }
    };
}

/// Macro for send [`Error`] to client and `return` from current function.
///
/// `$error_code` - some object which can tranform into [`Error`] by `Into`
/// trait.
///
/// `$response` - is type of response ([`GetResponse`], [`Response`]
/// etc).
macro_rules! send_error_response {
    ($ctx:tt, $sink:tt, $error_code:expr, $response:ty) => {
        let mut response = <$response>::new();
        let error: Error = $error_code.into();
        response.set_error(error);

        $ctx.spawn($sink.success(response).map_err(|e| {
            warn!("Error while sending Error response by gRPC. {:?}", e)
        }));

        return;
    };
}

/// Type alias for result of Create request.
type CreateResult =
    Result<Result<HashMap<String, String>, RoomError>, RoomServiceError>;

#[derive(Clone)]
struct ControlApiService {
    room_service: Addr<RoomService>,
    app: AppContext,
}

impl ControlApiService {
    /// Implementation of `Create` method for `Room` element.
    pub fn create_room(
        &mut self,
        req: &CreateRequest,
        local_uri: LocalUri<IsRoomId>,
    ) -> impl Future<Item = CreateResult, Error = ControlApiError> {
        let room_id = local_uri.take_room_id();

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
            self.room_service
                .send(StartRoom(room_id, room))
                .map_err(ControlApiError::from)
                .map(move |r| r.map(|_| Ok(sid))),
        )
    }

    /// Implementation of `Create` method for `Member` element.
    pub fn create_member(
        &mut self,
        req: &CreateRequest,
        local_uri: LocalUri<IsMemberId>,
    ) -> impl Future<Item = CreateResult, Error = ControlApiError> {
        let spec = fut_try!(MemberSpec::try_from(req.get_member()));

        let (member_id, room_uri) = local_uri.take_member_id();
        let room_id = room_uri.take_room_id();

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
            self.room_service
                .send(CreateMemberInRoom {
                    room_id,
                    member_id,
                    spec,
                })
                .map_err(ControlApiError::from)
                .map(|r| r.map(|r| r.map(|_| sids))),
        )
    }

    /// Implementation of `Create` method for `WebRtcPublishEndpoint` and
    /// `WebRtcPlayEndpoint` elements.
    pub fn create_endpoint(
        &mut self,
        req: &CreateRequest,
        local_uri: LocalUri<IsEndpointId>,
    ) -> impl Future<Item = CreateResult, Error = ControlApiError> {
        let endpoint = fut_try!(Endpoint::try_from(req));
        let (endpoint_id, member_uri) = local_uri.take_endpoint_id();
        let (member_id, room_uri) = member_uri.take_member_id();
        let room_id = room_uri.take_room_id();

        Either::A(
            self.room_service
                .send(CreateEndpointInRoom {
                    room_id: room_id,
                    member_id: member_id,
                    endpoint_id: endpoint_id,
                    spec: endpoint,
                })
                .map_err(ControlApiError::from)
                .map(|r| r.map(|r| r.map(|_| HashMap::new()))),
        )
    }
}

/// Generate [`Response`] for `Create` method of all elements.
fn get_response_for_create(
    result: Result<CreateResult, ControlApiError>,
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

impl ControlApi for ControlApiService {
    /// Implementation for `Create` method of gRPC control API.
    fn create(
        &mut self,
        ctx: RpcContext,
        req: CreateRequest,
        sink: UnarySink<Response>,
    ) {
        let local_uri = parse_local_uri!(req.get_id(), ctx, sink, Response);

        match local_uri {
            LocalUriType::Room(local_uri) => {
                if req.has_room() {
                    ctx.spawn(self.create_room(&req, local_uri).then(
                        move |r| {
                            sink.success(get_response_for_create(r))
                                .map_err(|_| ())
                        },
                    ));
                } else {
                    send_error_response!(
                        ctx,
                        sink,
                        ErrorCode::ElementIdForRoomButElementIsNot(
                            req.get_id().to_string(),
                        ),
                        Response
                    );
                }
            }
            LocalUriType::Member(local_uri) => {
                if req.has_member() {
                    ctx.spawn(self.create_member(&req, local_uri).then(
                        move |r| {
                            sink.success(get_response_for_create(r)).map_err(
                                |e| {
                                    warn!(
                                        "Error while sending Create response \
                                         by gRPC. {:?}",
                                        e
                                    )
                                },
                            )
                        },
                    ));
                } else {
                    send_error_response!(
                        ctx,
                        sink,
                        ErrorCode::ElementIdForMemberButElementIsNot(
                            req.get_id().to_string(),
                        ),
                        Response
                    );
                }
            }
            LocalUriType::Endpoint(local_uri) => {
                if req.has_webrtc_pub() || req.has_webrtc_play() {
                    ctx.spawn(self.create_endpoint(&req, local_uri).then(
                        move |r| {
                            sink.success(get_response_for_create(r)).map_err(
                                |e| {
                                    warn!(
                                        "Error while sending Create response \
                                         by gRPC. {:?}",
                                        e
                                    )
                                },
                            )
                        },
                    ));
                } else {
                    send_error_response!(
                        ctx,
                        sink,
                        ErrorCode::ElementIdForEndpointButElementIsNot(
                            req.get_id().to_string(),
                        ),
                        Response
                    );
                }
            }
        }

        // TODO
        //        if local_uri.is_room_uri() {
        //        } else if local_uri.is_member_uri() {
        //        } else if local_uri.is_endpoint_uri() {
        //        } else {
        //            send_error_response!(
        //                ctx,
        //                sink,
        //
        // ErrorCode::InvalidElementUri(req.get_id().to_string()),
        //                Response
        //            );
        //        }
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

            match uri {
                LocalUriType::Room(uri) => {
                    delete_room_futs.push(
                        self.room_service.send(DeleteRoom(uri.take_room_id())),
                    );
                }
                LocalUriType::Member(uri) => {
                    let (member_id, room_uri) = uri.take_member_id();
                    let room_id = room_uri.take_room_id();
                    delete_member_futs.push(
                        self.room_service
                            .send(DeleteMemberFromRoom { room_id, member_id }),
                    );
                }
                LocalUriType::Endpoint(uri) => {
                    let (endpoint_id, member_uri) = uri.take_endpoint_id();
                    let (member_id, room_uri) = member_uri.take_member_id();
                    let room_id = room_uri.take_room_id();
                    delete_endpoints_futs.push(self.room_service.send(
                        DeleteEndpointFromMember {
                            room_id,
                            member_id,
                            endpoint_id,
                        },
                    ));
                }
            }
            // TODO
            //            if uri.is_room_uri() {
            //            } else if uri.is_member_uri() {
            //            } else if uri.is_endpoint_uri() {
            //            } else {
            //                send_error_response!(
            //                    ctx,
            //                    sink,
            //                    ErrorCode::InvalidElementUri(id.to_string()),
            //                    Response
            //                );
            //            }
        }

        ctx.spawn(
            future::join_all(delete_room_futs)
                .join3(
                    future::join_all(delete_member_futs),
                    future::join_all(delete_endpoints_futs),
                )
                .then(move |result| {
                    let map_err_closure = |e| {
                        warn!(
                            "Error while sending Delete response by gRPC. {:?}",
                            e
                        )
                    };
                    match result {
                        Ok((member, endpoint, room)) => {
                            let results = member
                                .into_iter()
                                .chain(endpoint.into_iter())
                                .chain(room.into_iter());
                            for result in results {
                                if let Err(e) = result {
                                    let mut response = Response::new();
                                    let error: ErrorCode = e.into();
                                    response.set_error(error.into());
                                    return sink
                                        .success(response)
                                        .map_err(map_err_closure);
                                }
                            }

                            let mut response = Response::new();
                            response.set_sid(HashMap::new());
                            sink.success(response).map_err(map_err_closure)
                        }
                        Err(e) => {
                            warn!(
                                "Control API Delete method mailbox error. {:?}",
                                e
                            );
                            let mut response = Response::new();
                            let error: Error =
                                ErrorCode::UnknownError(format!("{:?}", e))
                                    .into();
                            response.set_error(error);
                            sink.success(response).map_err(map_err_closure)
                        }
                    }
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

            match local_uri {
                LocalUriType::Room(room_uri) => {
                    room_ids.push(room_uri.take_room_id());
                }
                LocalUriType::Member(member_uri) => {
                    let (member_id, room_uri) = member_uri.take_member_id();
                    let room_id = room_uri.take_room_id();
                    member_ids.push((room_id, member_id));
                }
                LocalUriType::Endpoint(endpoint_uri) => {
                    let (endpoint_id, member_uri) =
                        endpoint_uri.take_endpoint_id();
                    let (member_id, room_uri) = member_uri.take_member_id();
                    let room_id = room_uri.take_room_id();
                    endpoint_ids.push((room_id, member_id, endpoint_id));
                }
            }
            // TODO
            //            if local_uri.is_room_uri() {
            //            } else if local_uri.is_member_uri() {
            //            } else if local_uri.is_endpoint_uri() {
            //            } else {
            //                send_error_response!(
            //                    ctx,
            //                    sink,
            //                    ErrorCode::InvalidElementUri(id.to_string(),),
            //                    GetResponse
            //                );
            //            }
        }

        let room_fut = self.room_service.send(GetRoom(room_ids));
        let member_fut = self.room_service.send(GetMember(member_ids));
        let endpoint_fut = self.room_service.send(GetEndpoint(endpoint_ids));

        ctx.spawn(room_fut.join3(member_fut, endpoint_fut).then(|result| {
            let grpc_err_closure =
                |e| warn!("Error while sending Get response. {:?}", e);

            match result {
                Ok((room, member, endpoint)) => {
                    let mut elements = HashMap::new();
                    let mut elements_results = Vec::new();

                    let results = vec![room, member, endpoint];

                    for result in results {
                        match result {
                            Ok(o) => {
                                elements_results.push(o);
                            }
                            Err(e) => {
                                let mut response = GetResponse::new();
                                let error: ErrorCode = e.into();
                                response.set_error(error.into());
                                return sink
                                    .success(response)
                                    .map_err(grpc_err_closure);
                            }
                        }
                    }

                    let elements_results = elements_results
                        .into_iter()
                        .flat_map(std::iter::IntoIterator::into_iter);

                    for element in elements_results {
                        match element {
                            Ok((id, o)) => {
                                elements.insert(id, o);
                            }
                            Err(e) => {
                                let mut response = GetResponse::new();
                                let error: ErrorCode = e.into();
                                response.set_error(error.into());
                                return sink
                                    .success(response)
                                    .map_err(grpc_err_closure);
                            }
                        }
                    }

                    let mut response = GetResponse::new();
                    response.set_elements(elements);

                    sink.success(response).map_err(grpc_err_closure)
                }
                Err(e) => {
                    warn!("Control API Get method mailbox error. {:?}", e);
                    let mut response = GetResponse::new();
                    let error: Error =
                        ErrorCode::UnknownError(format!("{:?}", e)).into();
                    response.set_error(error);
                    sink.success(response).map_err(grpc_err_closure)
                }
            }
        }));
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
pub fn run(room_repo: Addr<RoomService>, app: AppContext) -> Addr<GrpcServer> {
    let bind_ip = app.config.grpc.bind_ip.to_string();
    let bind_port = app.config.grpc.bind_port;
    let cq_count = app.config.grpc.completion_queue_count;

    let service = create_control_api(ControlApiService {
        app,
        room_service: room_repo,
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
