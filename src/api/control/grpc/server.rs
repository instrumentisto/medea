//! Implementation of gRPC control API.

// Fix clippy needless_return in macro.
#![allow(clippy::needless_return)]

use std::{collections::HashMap, convert::TryFrom, sync::Arc};

use actix::{
    Actor, Addr, Arbiter, Context, Handler, MailboxError, ResponseFuture,
};
use failure::Fail;
use futures::future::{Either, Future};
use grpcio::{
    Environment, RpcContext, RpcStatus, RpcStatusCode, Server, ServerBuilder,
    UnarySink,
};
use medea_grpc_proto::{
    control::{
        ApplyRequest, CreateRequest, CreateResponse, Error, GetResponse,
        IdRequest, Response,
    },
    control_grpc::{create_control_api, ControlApi},
};

use crate::{
    api::{
        control::{
            local_uri::{
                IsEndpointId, IsMemberId, IsRoomId, LocalUri,
                LocalUriParseError, LocalUriType,
            },
            Endpoint, MemberSpec, RoomSpec, TryFromElementError,
            TryFromProtobufError,
        },
        error_codes::{ErrorCode, ErrorResponse},
    },
    log::prelude::*,
    signalling::{
        room::RoomError,
        room_service::{
            CreateEndpointInRoom, CreateMemberInRoom, DeleteElements,
            RoomService, RoomServiceError, StartRoom,
        },
    },
    AppContext,
};

use crate::{
    api::control::{MemberId, RoomId},
    shutdown::ShutdownGracefully,
    signalling::room_service::Get,
};

#[derive(Debug, Fail)]
pub enum ControlApiError {
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
        match LocalUriType::try_from($uri.as_ref()) {
            Ok(o) => o,
            Err(e) => {
                let error: ErrorResponse = e.into();
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
    fn get_sid(
        &self,
        room_id: &RoomId,
        member_id: &MemberId,
        credentials: &str,
    ) -> String {
        format!(
            "{}/{}/{}/{}",
            self.app.config.server.http.public_url,
            room_id,
            member_id,
            credentials
        )
    }

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
                let uri = self.get_sid(&room_id, &id, member.credentials());

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

        let sid = self.get_sid(&room_id, &member_id, spec.credentials());
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
                    room_id,
                    member_id,
                    endpoint_id,
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
) -> CreateResponse {
    let error: ErrorResponse = match result {
        Ok(r) => match r {
            Ok(r) => match r {
                Ok(sid) => {
                    let mut response = CreateResponse::new();
                    response.set_sid(sid);
                    return response;
                }
                Err(e) => e.into(),
            },
            Err(e) => e.into(),
        },
        Err(e) => e.into(),
    };

    let mut error_response = CreateResponse::new();
    error_response.set_error(error.into());
    error_response
}

impl ControlApi for ControlApiService {
    // TODO: just send Vec<LocalUri>, see fn delete()
    /// Implementation for `Create` method of gRPC control API.
    fn create(
        &mut self,
        ctx: RpcContext,
        req: CreateRequest,
        sink: UnarySink<CreateResponse>,
    ) {
        let local_uri =
            parse_local_uri!(req.get_id(), ctx, sink, CreateResponse);

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
                        ErrorResponse::new(
                            ErrorCode::ElementIdForRoomButElementIsNot,
                            &req.get_id(),
                        ),
                        CreateResponse
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
                        ErrorResponse::new(
                            ErrorCode::ElementIdForMemberButElementIsNot,
                            &req.get_id(),
                        ),
                        CreateResponse
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
                        ErrorResponse::new(
                            ErrorCode::ElementIdForEndpointButElementIsNot,
                            &req.get_id(),
                        ),
                        CreateResponse
                    );
                }
            }
        }
    }

    /// Implementation for `Apply` method of gRPC control API.
    fn apply(
        &mut self,
        ctx: RpcContext,
        _req: ApplyRequest,
        sink: UnarySink<Response>,
    ) {
        ctx.spawn(
            sink.fail(RpcStatus::new(
                RpcStatusCode::Unimplemented,
                Some("Apply method currently is unimplemented.".to_string()),
            ))
            .map(|_| {
                info!(
                    "An unimplemented gRPC Control API method 'Apply' was \
                     called."
                );
            })
            .map_err(|e| {
                warn!("Unimplemented method Apply error: {:?}", e);
            }),
        );
    }

    /// Implementation for `Delete` method of gRPC control API.
    fn delete(
        &mut self,
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<Response>,
    ) {
        let mut delete_elements = DeleteElements::new();

        for id in req.get_id() {
            let uri: LocalUriType = parse_local_uri!(id, ctx, sink, Response);
            delete_elements.add_uri(uri);
        }

        let delete_elements = match delete_elements.validate() {
            Ok(validated) => validated,
            Err(err) => {
                send_error_response!(
                    ctx,
                    sink,
                    ErrorResponse::from(err),
                    Response
                );
            }
        };

        ctx.spawn(
            self.room_service
                .send(delete_elements)
                .then(move |result| {
                    match result {
                        Ok(result) => match result {
                            Ok(_) => sink.success(Response::new()),
                            Err(e) => {
                                let mut response = Response::new();
                                response
                                    .set_error(ErrorResponse::from(e).into());
                                sink.success(response)
                            }
                        },
                        Err(e) => {
                            let mut response = Response::new();

                            // TODO: dont use unknown, add some special err for
                            // all       mailbox
                            // errs, Unavailable("ActorName") or
                            // something
                            let error: Error =
                                ErrorResponse::unknown(&format!("{:?}", e))
                                    .into();
                            response.set_error(error);

                            sink.success(response)
                        }
                    }
                })
                .map_err(|e| {
                    warn!(
                        "Error while sending response on Delete request by \
                         gRPC: {:?}",
                        e
                    )
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
        let mut uris = Vec::new();
        for id in req.get_id() {
            let local_uri = parse_local_uri!(id, ctx, sink, GetResponse);
            uris.push(local_uri);
        }
        ctx.spawn(
            self.room_service
                .send(Get(uris))
                .then(|mailbox_result| match mailbox_result {
                    Ok(room_service_result) => match room_service_result {
                        Ok(elements) => {
                            let mut response = GetResponse::new();
                            response.set_elements(
                                elements
                                    .into_iter()
                                    .map(|(id, value)| (id.to_string(), value))
                                    .collect(),
                            );
                            sink.success(response)
                        }
                        Err(e) => {
                            let mut response = GetResponse::new();
                            response.set_error(ErrorResponse::from(e).into());
                            sink.success(response)
                        }
                    },
                    Err(e) => {
                        let mut response = GetResponse::new();
                        response.set_error(ErrorResponse::unknown(&e).into());
                        sink.success(response)
                    }
                })
                .map_err(|e| {
                    warn!(
                        "Error while sending response on Get request by gRPC: \
                         {:?}",
                        e
                    )
                }),
        );
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
}

impl Handler<ShutdownGracefully> for GrpcServer {
    type Result = ResponseFuture<(), ()>;

    fn handle(
        &mut self,
        _: ShutdownGracefully,
        _: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "gRPC server received ShutdownGracefully message so shutting down.",
        );
        Box::new(self.server.shutdown().map_err(|e| warn!("{:?}", e)))
    }
}

/// Run gRPC server in actix actor.
pub fn run(room_repo: Addr<RoomService>, app: AppContext) -> Addr<GrpcServer> {
    let bind_ip = app.config.server.grpc.bind_ip.to_string();
    let bind_port = app.config.server.grpc.bind_port;
    let cq_count = app.config.server.grpc.completion_queue_count;

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
